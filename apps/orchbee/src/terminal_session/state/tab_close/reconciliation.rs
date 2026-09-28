use super::{
    AttachmentKey, CloseCarrier, CloseFlightKey, CloseOperation, CloseOperationKind, CloseResult,
    CloseSelection, CloseState, CloseStateError, NativeTerminalBinding, WorkbenchRequestId,
    WorkbenchTabCloseAttemptGeneration,
};
use crate::terminal_session::{
    state::NativeSessionState, workbench_projection::ClosedTabExpectation,
};
use orchbee_workbench_protocol::{WorkbenchTabCloseReadOutcome, WorkbenchTabCloseStatus};
use std::time::Instant;

pub(in crate::terminal_session) enum CloseLeaseAcknowledgement {
    Exchange(CloseFlightKey),
    Reconciliation(Box<CloseReconciliationMarker>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct CloseReconciliationOrigin {
    key: CloseFlightKey,
    operation: WorkbenchRequestId,
    attempt_generation: WorkbenchTabCloseAttemptGeneration,
    expectation: ClosedTabExpectation,
    selection: CloseSelection,
}

impl CloseReconciliationOrigin {
    pub(in crate::terminal_session) const fn key(&self) -> &CloseFlightKey {
        &self.key
    }
    #[cfg(test)]
    pub(in crate::terminal_session) const fn operation(&self) -> &WorkbenchRequestId {
        &self.operation
    }
    #[cfg(test)]
    pub(in crate::terminal_session) const fn attempt_generation(
        &self,
    ) -> WorkbenchTabCloseAttemptGeneration {
        self.attempt_generation
    }
    pub(in crate::terminal_session) const fn expectation(&self) -> &ClosedTabExpectation {
        &self.expectation
    }
    pub(in crate::terminal_session) const fn selection(&self) -> &CloseSelection {
        &self.selection
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct CloseReconciliationMarker {
    origin: CloseReconciliationOrigin,
    snapshot_request_id: WorkbenchRequestId,
    deadline: Instant,
    membership_recovery: bool,
}

impl CloseReconciliationMarker {
    pub(in crate::terminal_session) const fn origin(&self) -> &CloseReconciliationOrigin {
        &self.origin
    }
    pub(in crate::terminal_session) const fn snapshot_request_id(&self) -> &WorkbenchRequestId {
        &self.snapshot_request_id
    }
    pub(in crate::terminal_session) const fn deadline(&self) -> Instant {
        self.deadline
    }
    pub(in crate::terminal_session) const fn membership_recovery(&self) -> bool {
        self.membership_recovery
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct DefinitiveClosedAttachment {
    origin: CloseReconciliationOrigin,
    attachment_key: AttachmentKey,
}

impl DefinitiveClosedAttachment {
    pub(in crate::terminal_session) const fn attachment_key(&self) -> &AttachmentKey {
        &self.attachment_key
    }
    pub(in crate::terminal_session) const fn closed_expectation(&self) -> &ClosedTabExpectation {
        self.origin.expectation()
    }
}

enum ReconciliationProgress {
    Initial,
    Requested,
    WaitingManual,
    InFlight(Box<CloseReconciliationMarker>),
    Consumed(Box<CloseReconciliationMarker>),
}

// This token stays inside the membership install plan's exclusive native-state borrow.
pub(in crate::terminal_session::state) struct PreparedCloseConsumption {
    index: usize,
    consumed: RetainedReconciliation,
    restore_session: bool,
}

pub(super) struct RetainedReconciliation {
    origin: CloseReconciliationOrigin,
    progress: ReconciliationProgress,
    handoff_released: bool,
}

impl CloseState {
    pub(in crate::terminal_session::state) fn prepare_consumption(
        &self,
        marker: &CloseReconciliationMarker,
    ) -> Result<PreparedCloseConsumption, CloseStateError> {
        let index = self
            .record_index(marker.origin().key().target())
            .ok_or(CloseStateError::UnknownTarget)?;
        let retained = self.records[index]
            .reconciliation
            .as_ref()
            .ok_or(CloseStateError::UnknownTarget)?;
        if !matches!(&retained.progress, ReconciliationProgress::InFlight(current) if current.as_ref() == marker)
        {
            return Err(CloseStateError::OperationMismatch);
        }
        Ok(PreparedCloseConsumption {
            index,
            consumed: RetainedReconciliation {
                origin: retained.origin.clone(),
                progress: ReconciliationProgress::Consumed(Box::new(marker.clone())),
                handoff_released: false,
            },
            restore_session: matches!(&self.carrier, CloseCarrier::MembershipRecoveryPending(origin) if origin.as_ref() == marker.origin()),
        })
    }

    pub(in crate::terminal_session::state) fn commit_consumption(
        &mut self,
        prepared: PreparedCloseConsumption,
    ) {
        self.records[prepared.index].reconciliation = Some(prepared.consumed);
        if prepared.restore_session {
            self.carrier = CloseCarrier::Session;
        }
    }

    pub(super) fn retain_reconciliation_origin(
        &mut self,
        exchange: &CloseOperation,
        result: &CloseResult,
    ) -> Result<(), CloseStateError> {
        let (
            CloseOperationKind::Read { .. },
            CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                close_request_id,
                attempt_generation,
                status: WorkbenchTabCloseStatus::Closed { revision },
                ..
            }),
        ) = (exchange.kind(), result)
        else {
            return Ok(());
        };
        let target = exchange.key().target();
        let index = self
            .record_index(target)
            .ok_or(CloseStateError::UnknownTarget)?;
        let record = &mut self.records[index];
        if record.reconciliation.is_none() {
            record.reconciliation = Some(RetainedReconciliation {
                origin: CloseReconciliationOrigin {
                    key: exchange.key().clone(),
                    operation: close_request_id.clone(),
                    attempt_generation: *attempt_generation,
                    expectation: ClosedTabExpectation::new(
                        target.site_id().clone(),
                        target.terminal_context_id().clone(),
                        target.tab_id().clone(),
                        *revision,
                    ),
                    selection: record.selection.clone(),
                },
                progress: ReconciliationProgress::Initial,
                handoff_released: false,
            });
        }
        Ok(())
    }
}

impl NativeSessionState {
    #[cfg(test)]
    pub(in crate::terminal_session) fn has_retained_close_records(&self) -> bool {
        !self.close.records.is_empty()
    }
    pub(in crate::terminal_session) fn acknowledge_close_lease(
        &mut self,
        ack: CloseLeaseAcknowledgement,
    ) {
        match ack {
            CloseLeaseAcknowledgement::Exchange(key) => {
                if self
                    .close
                    .completed_handoff
                    .as_ref()
                    .is_some_and(|pending| pending.operation().key() == &key)
                {
                    self.close.completed_handoff = None;
                }
            }
            CloseLeaseAcknowledgement::Reconciliation(marker) => {
                if let Some(index) = self.close.record_index(marker.origin().key().target())
                    && let Some(retained) = &mut self.close.records[index].reconciliation
                    && matches!(&retained.progress, ReconciliationProgress::Consumed(current) if current == &marker)
                {
                    retained.handoff_released = true;
                }
            }
        }
        self.prune_consumed_close_records();
    }

    pub(in crate::terminal_session) fn release_joined_close_references(&mut self) {
        self.close.completed_handoff = None;
        for record in &mut self.close.records {
            if let Some(retained) = &mut record.reconciliation
                && matches!(retained.progress, ReconciliationProgress::Consumed(_))
            {
                retained.handoff_released = true;
            }
        }
        self.prune_consumed_close_records();
    }

    fn prune_consumed_close_records(&mut self) {
        let pending = self
            .close
            .pending
            .as_ref()
            .map(|operation| operation.key().target());
        let handed = self
            .close
            .completed_handoff
            .as_ref()
            .map(|receipt| receipt.operation().key().target());
        let confirmation = self
            .close
            .confirmation
            .as_ref()
            .map(|prompt| prompt.intent.selection().target());
        self.close.records.retain(|record| {
            let target = record.selection.target();
            !record.reconciliation.as_ref().is_some_and(|retained| {
                matches!(retained.progress, ReconciliationProgress::Consumed(_))
                    && retained.handoff_released
                    && self.cache.tab_index(target.tab_id()).is_none()
                    && pending != Some(target)
                    && handed != Some(target)
                    && confirmation != Some(target)
                    && !self.close.manual_pass.contains(target)
            })
        });
    }
    pub(in crate::terminal_session::state) fn close_membership_recovery_matches(
        &self,
        marker: &CloseReconciliationMarker,
    ) -> bool {
        marker.membership_recovery()
            && matches!(&self.close.carrier, CloseCarrier::MembershipRecoveryPending(origin) if origin.as_ref() == marker.origin())
            && matches!(self.phase, super::super::SessionPhase::Retired(_))
            && self.panes.iter().all(|record| !record.attached)
    }

    pub(in crate::terminal_session) fn close_reconciliation_requested_origin(
        &self,
    ) -> Option<CloseReconciliationOrigin> {
        self.close
            .records
            .iter()
            .filter_map(|record| record.reconciliation.as_ref())
            .find(|retained| matches!(retained.progress, ReconciliationProgress::Requested))
            .map(|retained| retained.origin.clone())
    }

    pub(in crate::terminal_session) fn retire_close_reconciliations(&mut self) {
        for record in &mut self.close.records {
            if let Some(retained) = &mut record.reconciliation
                && matches!(retained.progress, ReconciliationProgress::InFlight(_))
            {
                retained.progress = ReconciliationProgress::WaitingManual;
            }
        }
    }
    pub(in crate::terminal_session) fn plan_close_reconciliation(
        &self,
        request_id: WorkbenchRequestId,
        deadline: Instant,
    ) -> Result<CloseReconciliationMarker, CloseStateError> {
        if self.close.pending.is_some()
            || self.close.confirmation.is_some()
            || self.close.records.iter().any(|record| {
                record.reconciliation.as_ref().is_some_and(|retained| {
                    matches!(retained.progress, ReconciliationProgress::InFlight(_))
                })
            })
            || matches!(self.close.carrier, CloseCarrier::StatusOnly)
            || self.pending_refresh.is_some()
            || self.mutation_blocks_structure()
            || !(matches!(
                self.phase,
                super::super::SessionPhase::Ready(_)
                    | super::super::SessionPhase::Empty
                    | super::super::SessionPhase::SelectedWithoutAttachment(_)
                    | super::super::SessionPhase::RetryableAttachUnavailable(_)
            ) || (matches!(self.phase, super::super::SessionPhase::Retired(_))
                && matches!(
                    self.close.carrier,
                    CloseCarrier::MembershipRecoveryPending(_)
                )))
        {
            return Err(CloseStateError::Busy);
        }
        let retained = self
            .close
            .records
            .iter()
            .filter_map(|record| record.reconciliation.as_ref())
            .find(|retained| {
                let eligible = matches!(
                    retained.progress,
                    ReconciliationProgress::Initial | ReconciliationProgress::Requested
                );
                eligible
                    && match &self.close.carrier {
                        CloseCarrier::Session => true,
                        CloseCarrier::StatusOnly => false,
                        CloseCarrier::MembershipRecoveryPending(origin) => {
                            origin.as_ref() == &retained.origin
                                && matches!(retained.progress, ReconciliationProgress::Requested)
                        }
                    }
            })
            .ok_or(CloseStateError::Busy)?;
        if retained.origin.key().request_id() == &request_id {
            return Err(CloseStateError::OperationMismatch);
        }
        Ok(CloseReconciliationMarker {
            origin: retained.origin.clone(),
            snapshot_request_id: request_id,
            deadline,
            membership_recovery: matches!(
                self.close.carrier,
                CloseCarrier::MembershipRecoveryPending(_)
            ),
        })
    }

    pub(in crate::terminal_session) fn begin_close_reconciliation(
        &mut self,
        marker: &CloseReconciliationMarker,
    ) -> Result<(), CloseStateError> {
        if self
            .plan_close_reconciliation(marker.snapshot_request_id().clone(), marker.deadline())?
            != *marker
        {
            return Err(CloseStateError::OperationMismatch);
        }
        let index = self
            .close
            .record_index(marker.origin().key().target())
            .ok_or(CloseStateError::UnknownTarget)?;
        self.close.records[index]
            .reconciliation
            .as_mut()
            .ok_or(CloseStateError::UnknownTarget)?
            .progress = ReconciliationProgress::InFlight(Box::new(marker.clone()));
        Ok(())
    }

    pub(in crate::terminal_session) fn close_reconciliation_failed(
        &mut self,
        marker: &CloseReconciliationMarker,
    ) -> Result<(), CloseStateError> {
        let prepared = self.close.prepare_consumption(marker)?;
        self.close.records[prepared.index]
            .reconciliation
            .as_mut()
            .ok_or(CloseStateError::UnknownTarget)?
            .progress = ReconciliationProgress::WaitingManual;
        self.close
            .observation_failed(marker.origin().key().target());
        Ok(())
    }

    pub(in crate::terminal_session) fn request_live_close_reconciliation(&mut self) {
        if !matches!(self.close.carrier, CloseCarrier::Session) {
            return;
        }
        for record in &mut self.close.records {
            if let Some(retained) = &mut record.reconciliation
                && matches!(retained.progress, ReconciliationProgress::WaitingManual)
            {
                retained.progress = ReconciliationProgress::Requested;
            }
        }
    }

    pub(in crate::terminal_session) fn request_close_membership_recovery(
        &mut self,
    ) -> Result<(), CloseStateError> {
        if self.close.pending.is_some() || self.close.confirmation.is_some() {
            return Err(CloseStateError::Busy);
        }
        let index = match &self.close.carrier {
            CloseCarrier::StatusOnly => self.close.records.iter().position(|record| {
                record.displayed
                    && !record.unavailable
                    && record.closed_revision.is_some()
                    && record.reconciliation.as_ref().is_some_and(|retained| {
                        !matches!(
                            retained.progress,
                            ReconciliationProgress::InFlight(_)
                                | ReconciliationProgress::Consumed(_)
                        )
                    })
            }),
            CloseCarrier::MembershipRecoveryPending(origin) => {
                self.close.record_index(origin.key().target())
            }
            CloseCarrier::Session => return Err(CloseStateError::NotDisplayed),
        }
        .ok_or(CloseStateError::NotDisplayed)?;
        let retained = self.close.records[index]
            .reconciliation
            .as_mut()
            .ok_or(CloseStateError::UnknownTarget)?;
        if matches!(
            retained.progress,
            ReconciliationProgress::InFlight(_) | ReconciliationProgress::Consumed(_)
        ) {
            return Err(CloseStateError::Busy);
        }
        let origin = Box::new(retained.origin.clone());
        retained.progress = ReconciliationProgress::Requested;
        self.close.carrier = CloseCarrier::MembershipRecoveryPending(origin);
        self.close.invalidate_display();
        Ok(())
    }

    pub(in crate::terminal_session) fn refuse_close_reconciliation_admission(
        &mut self,
        origin: &CloseReconciliationOrigin,
    ) {
        if let Some(index) = self.close.record_index(origin.key().target())
            && let Some(retained) = &mut self.close.records[index].reconciliation
            && &retained.origin == origin
            && matches!(retained.progress, ReconciliationProgress::Requested)
        {
            retained.progress = ReconciliationProgress::WaitingManual;
        }
    }

    pub(in crate::terminal_session) fn close_reconciliation_origin(
        &self,
        key: &CloseFlightKey,
    ) -> Option<&CloseReconciliationOrigin> {
        let record = &self.close.records[self.close.record_index(key.target())?];
        let retained = record.reconciliation.as_ref()?;
        (retained.origin.key() == key
            && !matches!(retained.progress, ReconciliationProgress::Consumed(_)))
        .then_some(&retained.origin)
    }

    pub(in crate::terminal_session) fn definitive_closed_attachment(
        &self,
        origin: &CloseReconciliationOrigin,
        attachment: &AttachmentKey,
        binding: Option<&NativeTerminalBinding>,
    ) -> Option<DefinitiveClosedAttachment> {
        let retained = self.close_reconciliation_origin(origin.key())?;
        if retained != origin
            || origin.selection().attachment() != Some(attachment)
            || origin.selection().binding() != binding
            || origin.key().target().tab_id() != attachment.tab_id()
        {
            return None;
        }
        Some(DefinitiveClosedAttachment {
            origin: origin.clone(),
            attachment_key: attachment.clone(),
        })
    }

    #[cfg(test)]
    pub(in crate::terminal_session) fn definitive_closed_attachment_for(
        &self,
        attachment: &AttachmentKey,
        binding: Option<&NativeTerminalBinding>,
    ) -> Option<DefinitiveClosedAttachment> {
        self.close
            .records
            .iter()
            .filter_map(|record| record.reconciliation.as_ref())
            .find_map(|retained| {
                self.definitive_closed_attachment(&retained.origin, attachment, binding)
            })
    }
}
