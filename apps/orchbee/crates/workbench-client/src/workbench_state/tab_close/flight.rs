//! Owns captured close submission, read, and resume flights and retains their exact generation
//! and result evidence across connection-task retirement. It schedules observations without
//! issuing Server effects or inferring disposal from a lost response.

use super::{
    CloseConfirmationToken, CloseFlightKey, CloseIntent, CloseSelection, CloseState,
    CloseStateError,
};
use crate::exchange_values::workbench::{WorkbenchTabCloseAccepted, WorkbenchTabClosePrecommit};
use crate::workbench_state::{AttachmentKey, ClientNotice, ClientPhase, WorkbenchClientState};
use orchbee_workbench_protocol::{
    TabSetCloseAction, TabSetCloseCondition, TabSetCloseOutcome, WorkbenchRejectionReason,
    WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseReadOutcome,
    WorkbenchTabCloseResumeOutcome, WorkbenchTabId,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseReadMode {
    Initial,
    Manual,
    Closing,
    Disposed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CloseOperationKind {
    Close,
    /// The first Close of one Tab, which the Server admits only when nothing runs in it and
    /// otherwise answers that the user must confirm. Nothing is retained before it accepts, so the
    /// Tab leaves only then; `asked_at` is when the user asked, which the pending delay of an
    /// accepted close counts from.
    CloseIfIdle {
        asked_at: Instant,
    },
    Read {
        expected_operation: Option<WorkbenchRequestId>,
        mode: CloseReadMode,
    },
    Resume {
        operation: WorkbenchRequestId,
        generation: WorkbenchTabCloseAttemptGeneration,
    },
    /// One exchange of a confirmed set of several Tabs: its Start, or a Read that follows the set
    /// the Server accepted. The flight names the set's first member.
    TabSet(TabSetCloseAction),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CloseOperation {
    key: CloseFlightKey,
    selection: CloseSelection,
    kind: CloseOperationKind,
    deadline: Instant,
}

impl CloseOperation {
    pub(crate) fn new(
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
        }
    }

    pub(crate) const fn key(&self) -> &CloseFlightKey {
        &self.key
    }
    pub(crate) const fn selection(&self) -> &CloseSelection {
        &self.selection
    }
    pub(crate) const fn kind(&self) -> &CloseOperationKind {
        &self.kind
    }
    pub(crate) const fn deadline(&self) -> Instant {
        self.deadline
    }
    /// Whether this exchange closes `tab_id`: its own Tab, or any member a set Start names. A
    /// set's Read names its first member, as every member has left the Client's view by then.
    pub(crate) fn closes_tab(&self, tab_id: &WorkbenchTabId) -> bool {
        match &self.kind {
            CloseOperationKind::TabSet(TabSetCloseAction::Start { tab_ids, .. }) => {
                tab_ids.contains(tab_id)
            }
            CloseOperationKind::Close
            | CloseOperationKind::CloseIfIdle { .. }
            | CloseOperationKind::Read { .. }
            | CloseOperationKind::Resume { .. }
            | CloseOperationKind::TabSet(TabSetCloseAction::Read { .. }) => {
                self.selection.target().tab_id() == tab_id
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CloseResult {
    NotSubmitted,
    Precommit(WorkbenchTabClosePrecommit),
    Accepted(WorkbenchTabCloseAccepted),
    Read(WorkbenchTabCloseReadOutcome),
    Resume(WorkbenchTabCloseResumeOutcome),
    TabSet(TabSetCloseOutcome),
    ObservationUnavailable,
    Ambiguous,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingCloseResult {
    operation: CloseOperation,
    result: CloseResult,
}

pub(crate) enum CloseWorkerReturn {
    Returned(Option<PendingCloseResult>),
    Missing {
        queued: Option<CloseFlightKey>,
        deposited: Option<PendingCloseResult>,
    },
}

impl PendingCloseResult {
    pub(crate) const fn new(operation: CloseOperation, result: CloseResult) -> Self {
        Self { operation, result }
    }
    pub(crate) const fn operation(&self) -> &CloseOperation {
        &self.operation
    }
    pub(crate) const fn result(&self) -> &CloseResult {
        &self.result
    }
}

// Closing probes Host process disposal, which has no push source. This bounded observation
// also settles close receipts; it is not a substitute for membership publication events.
const READ_INTERVAL: Duration = Duration::from_millis(250);

impl CloseState {
    /// One publication lets the next status read skip its interval, so a close the Server has
    /// just published settles at once. Reads stay one at a time, and a read clears the wake.
    pub(crate) fn wake_for_publication(&mut self) {
        self.publication_wake = !self.records.is_empty();
        self.wake_set_for_publication();
    }

    fn read_candidate(&self, now: Instant) -> Option<(usize, CloseReadMode)> {
        if self.pending.is_some()
            || self.confirmation.is_some()
            || matches!(
                self.carrier,
                super::CloseCarrier::MembershipRecoveryPending(_)
            )
            || (!self.publication_wake
                && self
                    .last_read_admission
                    .is_some_and(|last| now.saturating_duration_since(last) < READ_INTERVAL))
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
        match operation.kind() {
            CloseOperationKind::CloseIfIdle { asked_at } => {
                return self.apply_close_if_idle(operation, *asked_at, result);
            }
            CloseOperationKind::TabSet(_) => return self.apply_set_result(operation, result),
            CloseOperationKind::Close
            | CloseOperationKind::Read { .. }
            | CloseOperationKind::Resume { .. } => {}
        }
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
                    if super::set::closed(status) {
                        self.set_member_closed(target);
                    }
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
            ) => self.retain_uncertain(index, operation),
            _ => return Err(CloseStateError::OperationMismatch),
        }
        Ok(())
    }

    /// The exchange may have reached the Server, so its close is followed by reads and never sent
    /// again.
    fn retain_uncertain(&mut self, index: usize, operation: &CloseOperation) {
        let token = self.token().map(super::CloseStatusToken);
        let record = &mut self.records[index];
        record.uncertain = true;
        record.uncertain_exchange = Some(operation.clone());
        record.unavailable = true;
        record.displayed = false;
        record.status_token = token;
    }

    /// Settles a Close the Server could admit unasked. Accepted, it is retained as a confirmed
    /// Close is, so only now does the Tab leave; a required confirmation presents the armed one;
    /// any other refusal withdraws it. An unknown outcome is followed like any other and never
    /// asked again, since asking could close the Tab twice.
    fn apply_close_if_idle(
        &mut self,
        operation: &CloseOperation,
        asked_at: Instant,
        result: &CloseResult,
    ) -> Result<(), CloseStateError> {
        if confirmation_required(operation, result) {
            return Ok(());
        }
        let target = operation.key().target();
        let armed = self
            .confirmation
            .as_ref()
            .is_some_and(|prompt| prompt.intent.selection() == operation.selection());
        match result {
            CloseResult::NotSubmitted | CloseResult::Precommit(_) => {}
            CloseResult::Accepted(receipt) => {
                self.retain_submission(operation.selection().clone(), asked_at)?;
                self.record_acceptance(target, receipt)?;
                let index = self
                    .record_index(target)
                    .ok_or(CloseStateError::UnknownTarget)?;
                self.records[index].initial_read_pending = self.records[index].status.is_none();
            }
            CloseResult::Ambiguous => {
                self.retain_submission(operation.selection().clone(), asked_at)?;
                let index = self
                    .record_index(target)
                    .ok_or(CloseStateError::UnknownTarget)?;
                self.retain_uncertain(index, operation);
            }
            CloseResult::Read(_)
            | CloseResult::Resume(_)
            | CloseResult::TabSet(_)
            | CloseResult::ObservationUnavailable => {
                return Err(CloseStateError::OperationMismatch);
            }
        }
        if armed {
            self.cancel_confirmation();
        }
        Ok(())
    }
}

/// Whether the Server answered a conditional Close that the user must confirm it, which refuses
/// nothing: the armed confirmation presents.
fn confirmation_required(operation: &CloseOperation, result: &CloseResult) -> bool {
    matches!(
        (operation.kind(), result),
        (
            CloseOperationKind::CloseIfIdle { .. },
            CloseResult::Precommit(WorkbenchTabClosePrecommit::Rejected {
                reason: WorkbenchRejectionReason::ConfirmationRequired,
                ..
            })
        )
    )
}

impl WorkbenchClientState {
    pub(crate) fn close_recovery_available(&self) -> bool {
        self.close.pending.is_none()
            && self
                .close
                .records
                .iter()
                .any(|record| record.operation.is_some() || record.uncertain)
    }

    pub(crate) fn begin_close_status_recovery(&mut self) -> bool {
        if !matches!(self.phase, ClientPhase::Retired(_)) || !self.close_recovery_available() {
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

    pub(crate) const fn close_status_only(&self) -> bool {
        !matches!(self.close.carrier, super::CloseCarrier::Client)
    }

    pub(crate) fn has_close_read_candidate(&self, now: Instant) -> bool {
        self.close.read_candidate(now).is_some()
    }

    pub(crate) fn close_carrier_retired(&mut self) {
        self.close.lose_set();
        self.close.cancel_confirmation();
        self.close.invalidate_display();
        self.panel_close.invalidate_display();
        self.close.carrier = super::CloseCarrier::Client;
        for record in &mut self.close.records {
            record.unavailable = true;
            record.announced = true;
            record.initial_read_pending = false;
        }
    }

    pub(crate) fn finalize_close_worker(
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
                    CloseOperationKind::Close
                    | CloseOperationKind::CloseIfIdle { .. }
                    | CloseOperationKind::Resume { .. }
                    | CloseOperationKind::TabSet(TabSetCloseAction::Start { .. }) => {
                        CloseResult::Ambiguous
                    }
                    CloseOperationKind::Read { .. }
                    | CloseOperationKind::TabSet(TabSetCloseAction::Read { .. }) => {
                        CloseResult::ObservationUnavailable
                    }
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

    pub(crate) fn plan_close_confirmation(
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
            .confirmation
            .as_ref()
            .ok_or(CloseStateError::NotDisplayed)?;
        let intent = self.close.confirmed_intent(prompt.token)?;
        if !self.close_confirmation_current(prompt) {
            return Err(CloseStateError::StaleConfirmation);
        }
        // One Tab is no set: it closes as the key closes it.
        if let CloseIntent::Set(capture) = &intent
            && capture.count() > 1
        {
            return self.plan_tab_set_start(
                capture,
                request_id,
                deadline,
                TabSetCloseCondition::Confirmed,
            );
        }
        let (selection, kind) = match intent {
            CloseIntent::Close(selection)
                if matches!(self.close.carrier, super::CloseCarrier::Client) =>
            {
                (selection, CloseOperationKind::Close)
            }
            CloseIntent::Set(capture)
                if matches!(self.close.carrier, super::CloseCarrier::Client) =>
            {
                (capture.first().clone(), CloseOperationKind::Close)
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

    /// Plans the conditional Close of the armed confirmation `token`, which stays hidden while the
    /// Server decides: one Tab's own Close, whichever route opened it, or the conditional Start of
    /// a set of several Tabs. A Resume always presents.
    pub(crate) fn plan_close_if_idle(
        &self,
        token: CloseConfirmationToken,
        request_id: WorkbenchRequestId,
        deadline: Instant,
        asked_at: Instant,
    ) -> Result<CloseOperation, CloseStateError> {
        if self.close.pending.is_some()
            || !matches!(self.close.carrier, super::CloseCarrier::Client)
        {
            return Err(CloseStateError::Busy);
        }
        let prompt = self
            .close
            .confirmation
            .as_ref()
            .filter(|prompt| prompt.token == token)
            .ok_or(CloseStateError::StaleConfirmation)?;
        if !self.close_confirmation_current(prompt) {
            return Err(CloseStateError::StaleConfirmation);
        }
        let kind = CloseOperationKind::CloseIfIdle { asked_at };
        match &prompt.intent {
            CloseIntent::Close(selection) => Ok(CloseOperation::new(
                request_id,
                selection.clone(),
                kind,
                deadline,
            )),
            CloseIntent::Set(capture) if capture.count() == 1 => Ok(CloseOperation::new(
                request_id,
                capture.first().clone(),
                kind,
                deadline,
            )),
            CloseIntent::Set(capture) => {
                self.plan_tab_set_start(capture, request_id, deadline, TabSetCloseCondition::IfIdle)
            }
            CloseIntent::Resume { .. } => Err(CloseStateError::NotDisplayed),
        }
    }

    pub(crate) fn plan_close_read(
        &self,
        request_id: WorkbenchRequestId,
        deadline: Instant,
        now: Instant,
    ) -> Result<CloseOperation, CloseStateError> {
        if self.pending_refresh.is_some()
            || self.mutation_blocks_structure()
            || !matches!(
                self.phase,
                ClientPhase::Ready(_)
                    | ClientPhase::Empty
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
                    | ClientPhase::Retired(_)
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

    pub(crate) fn begin_close_exchange(
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
                    .retain_submission(operation.selection().clone(), admitted_at)?;
                self.close.cancel_confirmation();
            }
            CloseOperationKind::TabSet(_) => self.begin_tab_set_exchange(operation, admitted_at)?,
            CloseOperationKind::CloseIfIdle { asked_at } => {
                let token = self
                    .close
                    .confirmation
                    .as_ref()
                    .map(|prompt| prompt.token)
                    .ok_or(CloseStateError::StaleConfirmation)?;
                if self.plan_close_if_idle(
                    token,
                    operation.key().request_id().clone(),
                    operation.deadline(),
                    *asked_at,
                )? != *operation
                {
                    return Err(CloseStateError::StaleConfirmation);
                }
                // The confirmation stays armed, and hidden, until the Server answers.
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
                self.close.publication_wake = false;
            }
        }
        self.close.pending = Some(operation.clone());
        self.notice = None;
        Ok(())
    }

    pub(crate) fn settle_close_exchange(
        &mut self,
        operation: &CloseOperation,
        result: &CloseResult,
    ) -> Result<(), CloseStateError> {
        if self.close.pending.as_ref() != Some(operation) {
            return Err(CloseStateError::OperationMismatch);
        }
        self.close.apply_result(operation, result)?;
        self.close.retain_reconciliation_origin(operation, result)?;
        self.close.pending = None;
        // The single worker cannot execute a successor before consuming this exact UI reply.
        self.close.completed_handoff =
            Some(PendingCloseResult::new(operation.clone(), result.clone()));
        let refused_start = if matches!(operation.kind(), CloseOperationKind::TabSet(_)) {
            self.settle_tab_set_exchange(result);
            super::set::tab_set_start_refused(operation, result)
        } else {
            matches!(
                result,
                CloseResult::NotSubmitted | CloseResult::Precommit(_)
            ) && !confirmation_required(operation, result)
        };
        let refused_resume = matches!(
            result,
            CloseResult::Resume(
                WorkbenchTabCloseResumeOutcome::Denied {}
                    | WorkbenchTabCloseResumeOutcome::Unavailable { .. }
                    | WorkbenchTabCloseResumeOutcome::Rejected { .. }
            )
        );
        if refused_resume || refused_start {
            self.notice = Some(ClientNotice::CloseRequestNotApplied);
        }
        Ok(())
    }

    pub(crate) fn retain_close_worker_result(
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
        if self.close.pending.as_ref() == Some(pending.operation()) {
            self.close.pending = None;
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn settled_close_identity(&self) -> Option<&CloseFlightKey> {
        self.close
            .completed_handoff
            .as_ref()
            .map(|result| result.operation().key())
    }

    #[cfg(test)]
    pub(crate) fn uncertain_close_identity(
        &self,
    ) -> Option<(&CloseFlightKey, Option<&WorkbenchRequestId>)> {
        self.close.records.iter().find_map(|record| {
            Some((
                record.uncertain_exchange.as_ref()?.key(),
                record.operation.as_ref(),
            ))
        })
    }

    pub(crate) fn request_close_manual_pass(&mut self) {
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

    pub(crate) fn close_suppresses_attachment(&self, key: &AttachmentKey) -> bool {
        self.close.set_leaving(key.tab_id())
            || self.close.records.iter().any(|record| {
                record.selection.target.tab_id() == key.tab_id()
                    && (record.operation.is_some() || record.uncertain)
            })
    }
}
