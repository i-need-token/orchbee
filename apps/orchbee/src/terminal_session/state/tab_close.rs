use super::AttachmentKey;
use crate::{
    local_client::workbench::WorkbenchTabCloseAccepted,
    terminal_session::workbench_projection::NativeTerminalBinding,
};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchLabel, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabClosePublicationReason,
    WorkbenchTabCloseStatus, WorkbenchTabId, WorkbenchTerminalContextId,
};

mod flight;
mod reconciliation;
pub(in crate::terminal_session) use flight::{
    CloseOperation, CloseOperationKind, CloseReadMode, CloseResult, CloseWorkerReturn,
    PendingCloseResult,
};
pub(super) use reconciliation::PreparedCloseConsumption;
pub(in crate::terminal_session) use reconciliation::{
    CloseReconciliationMarker, CloseReconciliationOrigin, DefinitiveClosedAttachment,
};

const MAXIMUM_CLOSE_RECORDS: usize = 256;
const DISPOSED_AUTO_ADMISSIONS: u8 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CloseConfirmationToken(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CloseStatusToken(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseConfirmationKind {
    Close,
    Resume,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseStatusKind {
    AwaitingStatus,
    Closing,
    CleanupBlocked,
    OutcomeUnknown,
    DisposedAwaitingPublication,
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct CloseTarget {
    site_id: SiteId,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
}

impl CloseTarget {
    pub(in crate::terminal_session) const fn new(
        site_id: SiteId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
    ) -> Self {
        Self {
            site_id,
            terminal_context_id,
            tab_id,
        }
    }

    pub(in crate::terminal_session) const fn site_id(&self) -> &SiteId {
        &self.site_id
    }
    pub(in crate::terminal_session) const fn terminal_context_id(
        &self,
    ) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }
    pub(in crate::terminal_session) const fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct CloseFlightKey {
    request_id: WorkbenchRequestId,
    target: CloseTarget,
}

impl CloseFlightKey {
    pub(in crate::terminal_session) const fn new(
        request_id: WorkbenchRequestId,
        target: CloseTarget,
    ) -> Self {
        Self { request_id, target }
    }
    pub(in crate::terminal_session) const fn request_id(&self) -> &WorkbenchRequestId {
        &self.request_id
    }
    pub(in crate::terminal_session) const fn target(&self) -> &CloseTarget {
        &self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct CloseSelection {
    target: CloseTarget,
    label: WorkbenchLabel,
    binding: NativeTerminalBinding,
    attachment: Option<AttachmentKey>,
    base_revision: WorkbenchRevision,
}

impl CloseSelection {
    pub(in crate::terminal_session) const fn new(
        target: CloseTarget,
        label: WorkbenchLabel,
        binding: NativeTerminalBinding,
        attachment: Option<AttachmentKey>,
        base_revision: WorkbenchRevision,
    ) -> Self {
        Self {
            target,
            label,
            binding,
            attachment,
            base_revision,
        }
    }
    pub(in crate::terminal_session) const fn target(&self) -> &CloseTarget {
        &self.target
    }
    pub(in crate::terminal_session) const fn base_revision(&self) -> WorkbenchRevision {
        self.base_revision
    }

    pub(in crate::terminal_session) const fn binding(&self) -> &NativeTerminalBinding {
        &self.binding
    }

    pub(in crate::terminal_session) const fn attachment(&self) -> Option<&AttachmentKey> {
        self.attachment.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum CloseIntent {
    Close(CloseSelection),
    Resume {
        selection: CloseSelection,
        operation: WorkbenchRequestId,
        generation: WorkbenchTabCloseAttemptGeneration,
        status_token: CloseStatusToken,
    },
}

impl CloseIntent {
    pub(in crate::terminal_session) const fn selection(&self) -> &CloseSelection {
        match self {
            Self::Close(selection) | Self::Resume { selection, .. } => selection,
        }
    }
    const fn kind(&self) -> CloseConfirmationKind {
        match self {
            Self::Close(_) => CloseConfirmationKind::Close,
            Self::Resume { .. } => CloseConfirmationKind::Resume,
        }
    }
}

struct CloseConfirmation {
    token: CloseConfirmationToken,
    intent: CloseIntent,
    shown: bool,
}

struct CloseRecord {
    selection: CloseSelection,
    operation: Option<WorkbenchRequestId>,
    accepted_revision: Option<WorkbenchRevision>,
    generation: Option<WorkbenchTabCloseAttemptGeneration>,
    status: Option<WorkbenchTabCloseStatus>,
    disposed: bool,
    closed_revision: Option<WorkbenchRevision>,
    uncertain: bool,
    uncertain_exchange: Option<CloseOperation>,
    unavailable: bool,
    disposed_auto_admissions: u8,
    status_token: Option<CloseStatusToken>,
    displayed: bool,
    initial_read_pending: bool,
    reconciliation: Option<reconciliation::RetainedReconciliation>,
}

impl CloseRecord {
    fn new(selection: CloseSelection) -> Self {
        Self {
            selection,
            operation: None,
            accepted_revision: None,
            generation: None,
            status: None,
            disposed: false,
            closed_revision: None,
            uncertain: false,
            uncertain_exchange: None,
            unavailable: false,
            disposed_auto_admissions: 0,
            status_token: None,
            displayed: false,
            initial_read_pending: false,
            reconciliation: None,
        }
    }

    fn resumable(&self) -> bool {
        !self.unavailable
            && matches!(
                self.status,
                Some(
                    WorkbenchTabCloseStatus::CleanupBlocked {
                        resumable: true,
                        ..
                    } | WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                        resumable: true,
                        ..
                    }
                )
            )
    }

    fn bounded_disposed_observation(&self) -> bool {
        matches!(
            self.status,
            Some(WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                reason: WorkbenchTabClosePublicationReason::TargetCleanupPending
                    | WorkbenchTabClosePublicationReason::Preparing,
                resumable: false,
            })
        )
    }

    fn kind(&self) -> CloseStatusKind {
        match self.status {
            None => CloseStatusKind::AwaitingStatus,
            Some(WorkbenchTabCloseStatus::Closing { .. }) => CloseStatusKind::Closing,
            Some(WorkbenchTabCloseStatus::CleanupBlocked { .. }) => CloseStatusKind::CleanupBlocked,
            Some(WorkbenchTabCloseStatus::OutcomeUnknown { .. }) => CloseStatusKind::OutcomeUnknown,
            Some(WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. }) => {
                CloseStatusKind::DisposedAwaitingPublication
            }
            Some(WorkbenchTabCloseStatus::Closed { .. }) => CloseStatusKind::Closed,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::terminal_session) struct CloseStatusView<'a> {
    record: &'a CloseRecord,
    token: CloseStatusToken,
}

impl<'a> CloseStatusView<'a> {
    pub(in crate::terminal_session) const fn token(self) -> CloseStatusToken {
        self.token
    }
    pub(in crate::terminal_session) fn target_label(self) -> &'a str {
        self.record.selection.label.as_str()
    }
    pub(in crate::terminal_session) fn status(self) -> CloseStatusKind {
        self.record.kind()
    }
    pub(in crate::terminal_session) fn resumable(self) -> bool {
        self.record.resumable()
    }
    pub(in crate::terminal_session) fn observation_paused(self) -> bool {
        self.record.bounded_disposed_observation()
            && self.record.disposed_auto_admissions == DISPOSED_AUTO_ADMISSIONS
    }
    pub(in crate::terminal_session) const fn unavailable(self) -> bool {
        self.record.unavailable
    }
}

pub(in crate::terminal_session) struct CloseConfirmationView<'a> {
    confirmation: &'a CloseConfirmation,
    status: Option<CloseStatusView<'a>>,
}

impl<'a> CloseConfirmationView<'a> {
    pub(in crate::terminal_session) const fn token(&self) -> CloseConfirmationToken {
        self.confirmation.token
    }
    pub(in crate::terminal_session) fn target_label(&self) -> &'a str {
        self.confirmation.intent.selection().label.as_str()
    }
    pub(in crate::terminal_session) const fn kind(&self) -> CloseConfirmationKind {
        self.confirmation.intent.kind()
    }
    pub(in crate::terminal_session) const fn resume_status(&self) -> Option<CloseStatusView<'a>> {
        self.status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum CloseStateError {
    Busy,
    NotResumable,
    NotDisplayed,
    StaleConfirmation,
    UnknownTarget,
    OperationMismatch,
    GenerationRegression,
    FactRegression,
    RevisionMismatch,
}

#[derive(Default)]
enum CloseCarrier {
    #[default]
    Session,
    StatusOnly,
    MembershipRecoveryPending(Box<CloseReconciliationOrigin>),
}

#[derive(Default)]
pub(in crate::terminal_session) struct CloseState {
    records: Vec<CloseRecord>,
    confirmation: Option<CloseConfirmation>,
    next_display_token: u64,
    pending: Option<CloseOperation>,
    completed_handoff: Option<PendingCloseResult>,
    last_read_admission: Option<std::time::Instant>,
    observation_cursor: usize,
    manual_pass: std::collections::VecDeque<CloseTarget>,
    carrier: CloseCarrier,
}

impl CloseState {
    fn token(&mut self) -> Option<u64> {
        self.next_display_token = self.next_display_token.checked_add(1)?;
        Some(self.next_display_token)
    }

    fn record_index(&self, target: &CloseTarget) -> Option<usize> {
        self.records
            .iter()
            .position(|record| record.selection.target == *target)
    }

    pub(in crate::terminal_session) fn open_confirmation(
        &mut self,
        selection: CloseSelection,
    ) -> Result<(), CloseStateError> {
        if self.confirmation.is_some() || self.pending.is_some() {
            return Err(CloseStateError::Busy);
        }
        let intent = if let Some(index) = self.record_index(&selection.target) {
            let record = &self.records[index];
            if !record.resumable() {
                return Err(CloseStateError::NotResumable);
            }
            if !record.displayed {
                return Err(CloseStateError::NotDisplayed);
            }
            CloseIntent::Resume {
                selection,
                operation: record
                    .operation
                    .clone()
                    .ok_or(CloseStateError::NotResumable)?,
                generation: record.generation.ok_or(CloseStateError::NotResumable)?,
                status_token: record.status_token.ok_or(CloseStateError::NotDisplayed)?,
            }
        } else {
            if self.records.len() == MAXIMUM_CLOSE_RECORDS {
                return Err(CloseStateError::Busy);
            }
            CloseIntent::Close(selection)
        };
        let token = CloseConfirmationToken(self.token().ok_or(CloseStateError::Busy)?);
        self.confirmation = Some(CloseConfirmation {
            token,
            intent,
            shown: false,
        });
        Ok(())
    }

    pub(in crate::terminal_session) fn cancel_confirmation(&mut self) {
        self.confirmation = None;
    }

    pub(in crate::terminal_session) fn confirmation(&self) -> Option<CloseConfirmationView<'_>> {
        let confirmation = self.confirmation.as_ref()?;
        let status = match &confirmation.intent {
            CloseIntent::Close(_) => None,
            CloseIntent::Resume {
                selection,
                status_token,
                ..
            } => self
                .status(&selection.target)
                .filter(|status| status.token == *status_token),
        };
        Some(CloseConfirmationView {
            confirmation,
            status,
        })
    }

    pub(in crate::terminal_session) fn status(
        &self,
        target: &CloseTarget,
    ) -> Option<CloseStatusView<'_>> {
        let record = &self.records[self.record_index(target)?];
        Some(CloseStatusView {
            token: record.status_token?,
            record,
        })
    }

    pub(in crate::terminal_session) fn latest_status(&self) -> Option<CloseStatusView<'_>> {
        self.records
            .iter()
            .filter_map(|record| {
                Some(CloseStatusView {
                    record,
                    token: record.status_token?,
                })
            })
            .max_by_key(|status| status.token.0)
    }

    pub(in crate::terminal_session) fn invalidate_display(&mut self) {
        for record in &mut self.records {
            record.displayed = false;
        }
        if let Some(confirmation) = &mut self.confirmation {
            confirmation.shown = false;
        }
    }

    pub(in crate::terminal_session) fn acknowledge_draw(
        &mut self,
        prompt: Option<CloseConfirmationToken>,
        status: Option<CloseStatusToken>,
    ) {
        for record in &mut self.records {
            record.displayed = status.is_some() && record.status_token == status;
        }
        let resume_displayed =
            self.confirmation
                .as_ref()
                .is_none_or(|confirmation| match &confirmation.intent {
                    CloseIntent::Close(_) => true,
                    CloseIntent::Resume {
                        selection,
                        operation,
                        generation,
                        status_token,
                    } => {
                        status == Some(*status_token)
                            && self.record_index(&selection.target).is_some_and(|index| {
                                let record = &self.records[index];
                                record.displayed
                                    && record.resumable()
                                    && record.operation.as_ref() == Some(operation)
                                    && record.generation == Some(*generation)
                                    && record.status_token == Some(*status_token)
                            })
                    }
                });
        if let Some(confirmation) = &mut self.confirmation {
            confirmation.shown = prompt == Some(confirmation.token) && resume_displayed;
        }
    }

    pub(in crate::terminal_session) fn confirmed_intent(
        &self,
        token: CloseConfirmationToken,
    ) -> Result<CloseIntent, CloseStateError> {
        let prompt = self
            .confirmation
            .as_ref()
            .filter(|prompt| prompt.token == token && prompt.shown)
            .ok_or(CloseStateError::NotDisplayed)?;
        if let CloseIntent::Resume {
            selection,
            operation,
            generation,
            status_token,
        } = &prompt.intent
        {
            let record = &self.records[self
                .record_index(&selection.target)
                .ok_or(CloseStateError::UnknownTarget)?];
            if !record.resumable()
                || !record.displayed
                || record.operation.as_ref() != Some(operation)
                || record.generation != Some(*generation)
                || record.status_token != Some(*status_token)
            {
                return Err(CloseStateError::StaleConfirmation);
            }
        }
        Ok(prompt.intent.clone())
    }

    pub(in crate::terminal_session) fn retain_submission(
        &mut self,
        selection: CloseSelection,
    ) -> Result<(), CloseStateError> {
        if self.record_index(&selection.target).is_some() {
            return Ok(());
        }
        if self.records.len() == MAXIMUM_CLOSE_RECORDS {
            return Err(CloseStateError::Busy);
        }
        self.records.push(CloseRecord::new(selection));
        Ok(())
    }

    pub(in crate::terminal_session) fn record_acceptance(
        &mut self,
        target: &CloseTarget,
        accepted: &WorkbenchTabCloseAccepted,
    ) -> Result<(), CloseStateError> {
        let index = self
            .record_index(target)
            .ok_or(CloseStateError::UnknownTarget)?;
        let record = &self.records[index];
        if accepted.terminal_context_id != target.terminal_context_id
            || accepted.tab_id != target.tab_id
            || record
                .operation
                .as_ref()
                .is_some_and(|operation| operation != &accepted.close_request_id)
        {
            return Err(CloseStateError::OperationMismatch);
        }
        if record
            .accepted_revision
            .is_some_and(|revision| revision != accepted.accepted_revision)
            || record
                .closed_revision
                .is_some_and(|revision| revision <= accepted.accepted_revision)
        {
            return Err(CloseStateError::RevisionMismatch);
        }
        let token = self.token().map(CloseStatusToken);
        let record = &mut self.records[index];
        record.operation = Some(accepted.close_request_id.clone());
        record.accepted_revision = Some(accepted.accepted_revision);
        record.uncertain = false;
        record.uncertain_exchange = None;
        record.status_token = token;
        record.displayed = false;
        Ok(())
    }

    pub(in crate::terminal_session) fn observe_available(
        &mut self,
        target: &CloseTarget,
        operation: WorkbenchRequestId,
        generation: WorkbenchTabCloseAttemptGeneration,
        status: WorkbenchTabCloseStatus,
    ) -> Result<(), CloseStateError> {
        let index = self
            .record_index(target)
            .ok_or(CloseStateError::UnknownTarget)?;
        let record = &self.records[index];
        if record
            .operation
            .as_ref()
            .is_some_and(|known| known != &operation)
        {
            return Err(CloseStateError::OperationMismatch);
        }
        if record.generation.is_some_and(|known| generation < known) {
            return Err(CloseStateError::GenerationRegression);
        }
        if record.closed_revision.is_some() && record.generation != Some(generation) {
            return Err(CloseStateError::FactRegression);
        }
        let disposed = matches!(
            status,
            WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. }
                | WorkbenchTabCloseStatus::Closed { .. }
        );
        if record.disposed && !disposed {
            return Err(CloseStateError::FactRegression);
        }
        if matches!(
            record.status,
            Some(WorkbenchTabCloseStatus::OutcomeUnknown { .. })
        ) && (record.status.as_ref() != Some(&status) || record.generation != Some(generation))
        {
            return Err(CloseStateError::FactRegression);
        }
        let closed = match &status {
            WorkbenchTabCloseStatus::Closed { revision } => Some(*revision),
            _ => None,
        };
        if record
            .closed_revision
            .is_some_and(|known| closed != Some(known))
            || closed.is_some_and(|revision| {
                record
                    .accepted_revision
                    .is_some_and(|accepted| revision <= accepted)
            })
        {
            return Err(CloseStateError::RevisionMismatch);
        }
        let token = self.token().map(CloseStatusToken);
        let record = &mut self.records[index];
        if record.generation.is_none_or(|known| generation > known) {
            record.disposed_auto_admissions = 0;
        }
        record.operation = Some(operation);
        record.generation = Some(generation);
        record.status = Some(status);
        record.disposed |= disposed;
        record.closed_revision = closed.or(record.closed_revision);
        record.uncertain = false;
        record.uncertain_exchange = None;
        record.unavailable = false;
        record.status_token = token;
        record.displayed = false;
        Ok(())
    }

    pub(in crate::terminal_session) fn charge_disposed_auto_admission(
        &mut self,
        target: &CloseTarget,
    ) -> Result<(), CloseStateError> {
        let index = self
            .record_index(target)
            .ok_or(CloseStateError::UnknownTarget)?;
        let record = &mut self.records[index];
        if !record.bounded_disposed_observation()
            || record.disposed_auto_admissions == DISPOSED_AUTO_ADMISSIONS
        {
            return Err(CloseStateError::Busy);
        }
        record.disposed_auto_admissions += 1;
        Ok(())
    }

    pub(in crate::terminal_session) fn observation_failed(&mut self, target: &CloseTarget) {
        if let Some(index) = self.record_index(target) {
            let record = &mut self.records[index];
            record.unavailable = true;
            record.displayed = false;
        }
        if self
            .confirmation
            .as_ref()
            .is_some_and(|prompt| prompt.intent.selection().target == *target)
        {
            self.confirmation = None;
        }
    }
}

impl super::NativeSessionState {
    pub(in crate::terminal_session) fn open_close_confirmation(
        &mut self,
    ) -> Result<(), CloseStateError> {
        if matches!(
            self.close.carrier,
            CloseCarrier::MembershipRecoveryPending(_)
        ) {
            return Err(CloseStateError::Busy);
        }
        if matches!(self.close.carrier, CloseCarrier::StatusOnly) {
            let selection = self
                .close
                .records
                .iter()
                .find(|record| record.displayed && record.resumable())
                .map(|record| record.selection.clone())
                .ok_or(CloseStateError::NotDisplayed)?;
            return self.close.open_confirmation(selection);
        }
        if self.pending_refresh_generation.is_some()
            || self.rename_draft_blocks_structure()
            || self.pending_move.is_some()
            || self.move_recovery.is_some()
            || !matches!(
                self.phase,
                super::SessionPhase::Ready(_)
                    | super::SessionPhase::SelectedWithoutAttachment(_)
                    | super::SessionPhase::RetryableAttachUnavailable(_)
            )
        {
            return Err(CloseStateError::Busy);
        }
        let selected = self.active().ok_or(CloseStateError::Busy)?;
        let tab = self
            .cache
            .tabs()
            .get(selected.index())
            .ok_or(CloseStateError::UnknownTarget)?;
        let target = CloseTarget::new(
            self.cache.site_id().clone(),
            self.cache.terminal_context_id().clone(),
            tab.tab_id().clone(),
        );
        let selection = CloseSelection::new(
            target,
            tab.tab_label().clone(),
            tab.binding().clone(),
            self.attached_key.clone(),
            self.cache.revision(),
        );
        self.close.open_confirmation(selection)
    }

    pub(in crate::terminal_session) fn cancel_close_confirmation(&mut self) {
        self.close.cancel_confirmation();
    }
    pub(in crate::terminal_session) fn invalidate_close_display(&mut self) {
        self.close.invalidate_display();
    }
    pub(in crate::terminal_session) fn acknowledge_close_draw(
        &mut self,
        prompt: Option<CloseConfirmationToken>,
        status: Option<CloseStatusToken>,
    ) {
        self.close.acknowledge_draw(prompt, status);
    }
}

impl<'a> super::NativeSessionView<'a> {
    pub(in crate::terminal_session) fn close_confirmation(
        &self,
    ) -> Option<CloseConfirmationView<'a>> {
        self.state.close.confirmation()
    }
    pub(in crate::terminal_session) fn close_status(&self) -> Option<CloseStatusView<'a>> {
        if let Some(selected) = self.state.active() {
            let target = CloseTarget::new(
                self.state.cache.site_id().clone(),
                self.state.cache.terminal_context_id().clone(),
                selected.key().tab_id().clone(),
            );
            if let Some(status) = self.state.close.status(&target) {
                return Some(status);
            }
        }
        self.state.close.latest_status()
    }
}
