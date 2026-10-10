use super::{CloseFlightKey, CloseIntent, CloseSelection, CloseState, CloseStateError};
use crate::local_client::workbench::{WorkbenchTabCloseAccepted, WorkbenchTabClosePrecommit};
use crate::terminal_session::state::{
    AttachmentKey, NativeSessionState, SessionNotice, SessionPhase,
};
use orchbee_workbench_protocol::{
    WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseReadOutcome,
    WorkbenchTabCloseResumeOutcome,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum CloseReadMode {
    Initial,
    Manual,
    Closing,
    Disposed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum CloseOperationKind {
    Close,
    Read {
        expected_operation: Option<WorkbenchRequestId>,
        mode: CloseReadMode,
    },
    Resume {
        operation: WorkbenchRequestId,
        generation: WorkbenchTabCloseAttemptGeneration,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct CloseOperation {
    key: CloseFlightKey,
    selection: CloseSelection,
    kind: CloseOperationKind,
    deadline: Instant,
    set_source: Option<std::sync::Arc<orchbee_workbench_protocol::Snapshot>>,
}

impl CloseOperation {
    pub(in crate::terminal_session) fn new(
        request_id: WorkbenchRequestId,
        selection: CloseSelection,
        kind: CloseOperationKind,
        deadline: Instant,
    ) -> Self {
        Self {
            key: CloseFlightKey::new(request_id, selection.target().clone()),
            selection,
            kind,
            deadline,
            set_source: None,
        }
    }

    pub(in crate::terminal_session) const fn key(&self) -> &CloseFlightKey {
        &self.key
    }
    pub(in crate::terminal_session) const fn selection(&self) -> &CloseSelection {
        &self.selection
    }
    pub(in crate::terminal_session) const fn kind(&self) -> &CloseOperationKind {
        &self.kind
    }
    pub(in crate::terminal_session) const fn deadline(&self) -> Instant {
        self.deadline
    }
    pub(super) fn with_set_source(
        mut self,
        source: std::sync::Arc<orchbee_workbench_protocol::Snapshot>,
    ) -> Self {
        self.set_source = Some(source);
        self
    }
    pub(in crate::terminal_session) fn set_source(
        &self,
    ) -> Option<&orchbee_workbench_protocol::Snapshot> {
        self.set_source.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum CloseResult {
    NotSubmitted,
    Precommit(WorkbenchTabClosePrecommit),
    Accepted(WorkbenchTabCloseAccepted),
    Read(WorkbenchTabCloseReadOutcome),
    Resume(WorkbenchTabCloseResumeOutcome),
    ObservationUnavailable,
    Ambiguous,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct PendingCloseResult {
    operation: CloseOperation,
    result: CloseResult,
}

pub(in crate::terminal_session) enum CloseWorkerReturn {
    Returned(Option<PendingCloseResult>),
    Missing {
        queued: Option<CloseFlightKey>,
        deposited: Option<PendingCloseResult>,
    },
}

impl PendingCloseResult {
    pub(in crate::terminal_session) const fn new(
        operation: CloseOperation,
        result: CloseResult,
    ) -> Self {
        Self { operation, result }
    }
    pub(in crate::terminal_session) const fn operation(&self) -> &CloseOperation {
        &self.operation
    }
    pub(in crate::terminal_session) const fn result(&self) -> &CloseResult {
        &self.result
    }
}

const READ_INTERVAL: Duration = Duration::from_millis(250);

impl CloseState {
    fn read_candidate(&self, now: Instant) -> Option<(usize, CloseReadMode)> {
        if self.pending.is_some()
            || self.confirmation.is_some()
            || matches!(
                self.carrier,
                super::CloseCarrier::MembershipRecoveryPending(_)
            )
            || self
                .last_read_admission
                .is_some_and(|last| now.saturating_duration_since(last) < READ_INTERVAL)
        {
            return None;
        }
        if let Some(target) = self.manual_pass.front()
            && let Some(index) = self.record_index(target)
        {
            return Some((index, CloseReadMode::Manual));
        }
        (0..self.records.len()).find_map(|offset| {
            let index = (self.observation_cursor + offset) % self.records.len();
            let record = &self.records[index];
            let mode = if record.initial_read_pending {
                CloseReadMode::Initial
            } else if record.unavailable {
                return None;
            } else if matches!(
                record.status,
                Some(orchbee_workbench_protocol::WorkbenchTabCloseStatus::Closing { .. })
            ) {
                CloseReadMode::Closing
            } else if record.bounded_disposed_observation()
                && record.disposed_auto_admissions < super::DISPOSED_AUTO_ADMISSIONS
            {
                CloseReadMode::Disposed
            } else {
                return None;
            };
            Some((index, mode))
        })
    }

    fn apply_result(
        &mut self,
        operation: &CloseOperation,
        result: &CloseResult,
    ) -> Result<(), CloseStateError> {
        let target = operation.key().target();
        let index = self
            .record_index(target)
            .ok_or(CloseStateError::UnknownTarget)?;
        match (operation.kind(), result) {
            (CloseOperationKind::Close, CloseResult::Accepted(receipt)) => {
                self.record_acceptance(target, receipt)?;
                self.records[index].initial_read_pending = self.records[index].status.is_none();
            }
            (CloseOperationKind::Close, CloseResult::NotSubmitted | CloseResult::Precommit(_)) => {
                let record = &self.records[index];
                if record.operation.is_none() && !record.disposed && !record.uncertain {
                    self.records.remove(index);
                    self.manual_pass.retain(|queued| queued != target);
                }
            }
            (
                CloseOperationKind::Read {
                    expected_operation, ..
                },
                CloseResult::Read(outcome),
            ) => match outcome {
                WorkbenchTabCloseReadOutcome::Available {
                    close_request_id,
                    terminal_context_id,
                    tab_id,
                    attempt_generation,
                    status,
                } => {
                    if terminal_context_id != target.terminal_context_id()
                        || tab_id != target.tab_id()
                        || expected_operation
                            .as_ref()
                            .is_some_and(|known| known != close_request_id)
                    {
                        return Err(CloseStateError::OperationMismatch);
                    }
                    self.observe_available(
                        target,
                        close_request_id.clone(),
                        *attempt_generation,
                        status.clone(),
                    )?;
                    self.records[index].initial_read_pending = false;
                }
                WorkbenchTabCloseReadOutcome::NotClosing {}
                    if self.records[index].operation.is_none() =>
                {
                    if self.records[index].uncertain {
                        self.records[index].initial_read_pending = false;
                        self.observation_failed(target);
                    } else {
                        self.records.remove(index);
                        self.manual_pass.retain(|queued| queued != target);
                    }
                }
                WorkbenchTabCloseReadOutcome::NotClosing {} => {
                    return Err(CloseStateError::FactRegression);
                }
                WorkbenchTabCloseReadOutcome::Denied {}
                | WorkbenchTabCloseReadOutcome::Unavailable { .. } => {
                    self.records[index].initial_read_pending = false;
                    self.observation_failed(target);
                }
            },
            (
                CloseOperationKind::Resume {
                    operation: expected_operation,
                    generation,
                },
                CloseResult::Resume(outcome),
            ) => {
                if self.records[index].operation.as_ref() != Some(expected_operation) {
                    return Err(CloseStateError::OperationMismatch);
                }
                match outcome {
                    WorkbenchTabCloseResumeOutcome::Resumed { attempt_generation }
                    | WorkbenchTabCloseResumeOutcome::Stale { attempt_generation } => {
                        if *attempt_generation <= *generation
                            || self.records[index]
                                .generation
                                .is_some_and(|known| *attempt_generation < known)
                        {
                            return Err(CloseStateError::GenerationRegression);
                        }
                        let token = self.token().map(super::CloseStatusToken);
                        let record = &mut self.records[index];
                        if record
                            .generation
                            .is_none_or(|known| *attempt_generation > known)
                        {
                            record.disposed_auto_admissions = 0;
                        }
                        record.generation = Some(*attempt_generation);
                        record.status = None;
                        record.status_token = token;
                        record.displayed = false;
                        record.initial_read_pending = true;
                        record.unavailable = false;
                    }
                    WorkbenchTabCloseResumeOutcome::Denied {}
                    | WorkbenchTabCloseResumeOutcome::Unavailable { .. }
                    | WorkbenchTabCloseResumeOutcome::Rejected { .. } => {
                        self.observation_failed(target)
                    }
                }
            }
            (
                CloseOperationKind::Read { .. },
                CloseResult::NotSubmitted | CloseResult::ObservationUnavailable,
            )
            | (CloseOperationKind::Resume { .. }, CloseResult::NotSubmitted) => {
                self.records[index].initial_read_pending = false;
                self.observation_failed(target);
            }
            (
                CloseOperationKind::Close | CloseOperationKind::Resume { .. },
                CloseResult::Ambiguous,
            ) => {
                let token = self.token().map(super::CloseStatusToken);
                let record = &mut self.records[index];
                record.uncertain = true;
                record.uncertain_exchange = Some(operation.clone());
                record.unavailable = true;
                record.displayed = false;
                record.status_token = token;
            }
            _ => return Err(CloseStateError::OperationMismatch),
        }
        Ok(())
    }
}

impl NativeSessionState {
    #[cfg(any(target_os = "linux", windows))]
    pub(in crate::terminal_session) fn close_recovery_available(&self) -> bool {
        self.close.pending.is_none()
            && self
                .close
                .records
                .iter()
                .any(|record| record.operation.is_some() || record.uncertain)
    }

    #[cfg(any(target_os = "linux", windows))]
    pub(in crate::terminal_session) fn begin_close_status_recovery(&mut self) -> bool {
        if !matches!(self.phase, SessionPhase::Retired(_)) || !self.close_recovery_available() {
            return false;
        }
        self.close.carrier = super::CloseCarrier::StatusOnly;
        self.close.invalidate_display();
        self.close.manual_pass.clear();
        self.close.manual_pass.extend(
            self.close
                .records
                .iter()
                .map(|record| record.selection.target.clone()),
        );
        true
    }

    pub(in crate::terminal_session) const fn close_status_only(&self) -> bool {
        !matches!(self.close.carrier, super::CloseCarrier::Session)
    }

    pub(in crate::terminal_session) fn has_close_read_candidate(&self, now: Instant) -> bool {
        self.close.read_candidate(now).is_some()
    }

    pub(in crate::terminal_session) fn close_carrier_retired(&mut self) {
        self.close.halt_set();
        self.close.cancel_confirmation();
        self.close.invalidate_display();
        self.panel_close.invalidate_display();
        self.close.carrier = super::CloseCarrier::Session;
        for record in &mut self.close.records {
            record.unavailable = true;
            record.initial_read_pending = false;
        }
    }

    pub(in crate::terminal_session) fn finalize_close_worker(
        &mut self,
        returned: CloseWorkerReturn,
    ) -> Result<(), CloseStateError> {
        let (missing, queued_proof, receipt) = match returned {
            CloseWorkerReturn::Returned(pending) => (false, None, pending),
            CloseWorkerReturn::Missing { queued, deposited } => (true, queued, deposited),
        };
        let retained = receipt.map_or(Ok(()), |pending| self.retain_close_worker_result(pending));
        if let Some(queued) = self.close.pending.take() {
            let result = if missing && queued_proof.as_ref() != Some(queued.key()) {
                match queued.kind() {
                    CloseOperationKind::Close | CloseOperationKind::Resume { .. } => {
                        CloseResult::Ambiguous
                    }
                    CloseOperationKind::Read { .. } => CloseResult::ObservationUnavailable,
                }
            } else {
                CloseResult::NotSubmitted
            };
            let settled = self.close.apply_result(&queued, &result);
            self.close_carrier_retired();
            return retained.and(settled);
        }
        self.close_carrier_retired();
        retained
    }

    pub(in crate::terminal_session) fn plan_close_confirmation(
        &self,
        request_id: WorkbenchRequestId,
        deadline: Instant,
    ) -> Result<CloseOperation, CloseStateError> {
        if self.close.pending.is_some() {
            return Err(CloseStateError::Busy);
        }
        if matches!(
            self.close.carrier,
            super::CloseCarrier::MembershipRecoveryPending(_)
        ) {
            return Err(CloseStateError::Busy);
        }
        let prompt = self
            .close
            .confirmation()
            .ok_or(CloseStateError::NotDisplayed)?;
        let intent = self.close.confirmed_intent(prompt.token())?;
        if let CloseIntent::Set(capture) = &intent {
            return self.plan_first_close_set(capture, request_id, deadline);
        }
        let selection = intent.selection();
        if matches!(self.close.carrier, super::CloseCarrier::Session) {
            let selected = self.active().ok_or(CloseStateError::StaleConfirmation)?;
            let tab = self
                .cache
                .tabs()
                .get(selected.index())
                .ok_or(CloseStateError::StaleConfirmation)?;
            let panel = tab
                .panel(selected.panel_index())
                .ok_or(CloseStateError::StaleConfirmation)?;
            if self.cache.site_id() != selection.target().site_id()
                || self.cache.terminal_context_id() != selection.target().terminal_context_id()
                || self.cache.revision() != selection.base_revision()
                || tab.tab_id() != selection.target().tab_id()
                || self.selected_presentation(panel).binding() != selection.binding()
                || self
                    .desired
                    .as_ref()
                    .map(crate::terminal_session::state::PaneKey::anchor)
                    != selection.attachment()
            {
                return Err(CloseStateError::StaleConfirmation);
            }
        }
        let (selection, kind) = match intent {
            CloseIntent::Close(selection)
                if matches!(self.close.carrier, super::CloseCarrier::Session) =>
            {
                (selection, CloseOperationKind::Close)
            }
            CloseIntent::Resume {
                selection,
                operation,
                generation,
                ..
            } => (
                selection,
                CloseOperationKind::Resume {
                    operation,
                    generation,
                },
            ),
            CloseIntent::Close(_) | CloseIntent::Set(_) => {
                return Err(CloseStateError::NotResumable);
            }
        };
        Ok(CloseOperation::new(request_id, selection, kind, deadline))
    }

    pub(in crate::terminal_session) fn plan_close_read(
        &self,
        request_id: WorkbenchRequestId,
        deadline: Instant,
        now: Instant,
    ) -> Result<CloseOperation, CloseStateError> {
        if self.pending_refresh.is_some()
            || self.mutation_blocks_structure()
            || !matches!(
                self.phase,
                SessionPhase::Ready(_)
                    | SessionPhase::Empty
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
                    | SessionPhase::Retired(_)
            )
        {
            return Err(CloseStateError::Busy);
        }
        let (index, mode) = self
            .close
            .read_candidate(now)
            .ok_or(CloseStateError::Busy)?;
        let record = &self.close.records[index];
        Ok(CloseOperation::new(
            request_id,
            record.selection.clone(),
            CloseOperationKind::Read {
                expected_operation: record.operation.clone(),
                mode,
            },
            deadline,
        ))
    }

    pub(in crate::terminal_session) fn begin_close_exchange(
        &mut self,
        operation: &CloseOperation,
        admitted_at: Instant,
    ) -> Result<(), CloseStateError> {
        if self.close.pending.is_some() {
            return Err(CloseStateError::Busy);
        }
        match operation.kind() {
            CloseOperationKind::Close | CloseOperationKind::Resume { .. } => {
                if self.plan_close_confirmation(
                    operation.key().request_id().clone(),
                    operation.deadline(),
                )? != *operation
                {
                    return Err(CloseStateError::StaleConfirmation);
                }
                self.close
                    .retain_submission(operation.selection().clone())?;
                if let Some(super::CloseConfirmation {
                    intent: CloseIntent::Set(capture),
                    ..
                }) = self.close.confirmation.as_ref()
                {
                    self.begin_first_close_set((**capture).clone(), operation);
                }
                self.close.cancel_confirmation();
            }
            CloseOperationKind::Read { mode, .. } => {
                if self.plan_close_read(
                    operation.key().request_id().clone(),
                    operation.deadline(),
                    admitted_at,
                )? != *operation
                {
                    return Err(CloseStateError::Busy);
                }
                if *mode == CloseReadMode::Disposed {
                    self.close
                        .charge_disposed_auto_admission(operation.key().target())?;
                }
                if *mode == CloseReadMode::Manual {
                    self.close.manual_pass.pop_front();
                }
                let index = self
                    .close
                    .record_index(operation.key().target())
                    .ok_or(CloseStateError::UnknownTarget)?;
                self.close.observation_cursor = (index + 1) % self.close.records.len();
                self.close.last_read_admission = Some(admitted_at);
            }
        }
        self.close.pending = Some(operation.clone());
        self.notice = None;
        Ok(())
    }

    pub(in crate::terminal_session) fn settle_close_exchange(
        &mut self,
        operation: &CloseOperation,
        result: &CloseResult,
    ) -> Result<(), CloseStateError> {
        if self.close.pending.as_ref() != Some(operation) {
            return Err(CloseStateError::OperationMismatch);
        }
        self.close.apply_result(operation, result)?;
        self.close.observe_set_result(operation, result);
        self.close.retain_reconciliation_origin(operation, result)?;
        self.close.pending = None;
        // The single worker cannot execute a successor before consuming this exact UI reply.
        self.close.completed_handoff =
            Some(PendingCloseResult::new(operation.clone(), result.clone()));
        if matches!(
            result,
            CloseResult::NotSubmitted
                | CloseResult::Precommit(_)
                | CloseResult::Resume(
                    WorkbenchTabCloseResumeOutcome::Denied {}
                        | WorkbenchTabCloseResumeOutcome::Unavailable { .. }
                        | WorkbenchTabCloseResumeOutcome::Rejected { .. }
                )
        ) {
            self.notice = Some(SessionNotice::CloseRequestNotApplied);
        }
        Ok(())
    }

    pub(in crate::terminal_session) fn retain_close_worker_result(
        &mut self,
        pending: PendingCloseResult,
    ) -> Result<(), CloseStateError> {
        if let Some(completed) = self.close.completed_handoff.as_ref()
            && completed.operation() == pending.operation()
        {
            return if completed.result() == pending.result() {
                Ok(())
            } else {
                Err(CloseStateError::FactRegression)
            };
        }
        let result = self
            .close
            .apply_result(pending.operation(), pending.result())
            .and_then(|()| {
                self.close
                    .retain_reconciliation_origin(pending.operation(), pending.result())
            });
        if result.is_ok() {
            self.close
                .observe_set_result(pending.operation(), pending.result());
        }
        if self.close.pending.as_ref() == Some(pending.operation()) {
            self.close.pending = None;
        }
        result
    }

    #[cfg(test)]
    pub(in crate::terminal_session) fn settled_close_identity(&self) -> Option<&CloseFlightKey> {
        self.close
            .completed_handoff
            .as_ref()
            .map(|result| result.operation().key())
    }

    #[cfg(test)]
    pub(in crate::terminal_session) fn uncertain_close_identity(
        &self,
    ) -> Option<(&CloseFlightKey, Option<&WorkbenchRequestId>)> {
        self.close.records.iter().find_map(|record| {
            Some((
                record.uncertain_exchange.as_ref()?.key(),
                record.operation.as_ref(),
            ))
        })
    }

    pub(in crate::terminal_session) fn request_close_manual_pass(&mut self) {
        if self.close.manual_pass.is_empty() {
            self.close.manual_pass.extend(
                self.close
                    .records
                    .iter()
                    .filter(|record| record.closed_revision.is_none())
                    .map(|record| record.selection.target.clone()),
            );
        }
    }

    pub(in crate::terminal_session) fn close_suppresses_attachment(
        &self,
        key: &AttachmentKey,
    ) -> bool {
        self.close.records.iter().any(|record| {
            record.selection.target.tab_id() == key.tab_id()
                && (record.operation.is_some() || record.uncertain)
        })
    }
}
