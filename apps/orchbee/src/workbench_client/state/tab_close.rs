use super::{AttachmentKey, ClientNotice, PaneKey};
use crate::{
    local_client::workbench::WorkbenchTabCloseAccepted,
    tui_presentation::{GeometrySettlement, PresentationPhase},
    workbench_client::workbench_projection::NativeTerminalBinding,
};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchLabel, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabClosePublicationReason,
    WorkbenchTabCloseStatus, WorkbenchTabId, WorkbenchTerminalContextId,
};

mod flight;
mod reconciliation;
mod set;
#[cfg(test)]
pub(in crate::workbench_client) use flight::CloseReadMode;
pub(in crate::workbench_client) use flight::{
    CloseOperation, CloseOperationKind, CloseResult, CloseWorkerReturn, PendingCloseResult,
};
pub(in crate::workbench_client) use reconciliation::CloseLeaseAcknowledgement;
pub(super) use reconciliation::PreparedCloseConsumption;
pub(in crate::workbench_client) use reconciliation::{
    CloseReconciliationMarker, CloseReconciliationOrigin, DefinitiveClosedAttachment,
};
pub(in crate::workbench_client) use set::CloseSetCapture;
pub(crate) use set::CloseSetStatus;
#[cfg(any(windows, test))]
pub(in crate::workbench_client) use set::{CloseSetKind, close_set_targets};

const MAXIMUM_CLOSE_RECORDS: usize = 256;
const DISPOSED_AUTO_ADMISSIONS: u8 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CloseConfirmationToken(u64);

impl CloseConfirmationToken {
    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

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
pub(in crate::workbench_client) struct CloseTarget {
    site_id: SiteId,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
}

impl CloseTarget {
    pub(in crate::workbench_client) const fn new(
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

    pub(in crate::workbench_client) const fn site_id(&self) -> &SiteId {
        &self.site_id
    }
    pub(in crate::workbench_client) const fn terminal_context_id(
        &self,
    ) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }
    pub(in crate::workbench_client) const fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }
}

/// Exact authority to continue one previously presented, resumable Close.
///
/// Only the close owner can mint this value, and only while the matching
/// status is genuinely displayed. A covering child may retain it, but every
/// use revalidates the live target, operation, attempt and status identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct TabCloseResumePermit {
    target: CloseTarget,
    operation: WorkbenchRequestId,
    generation: WorkbenchTabCloseAttemptGeneration,
    status_token: CloseStatusToken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct CloseFlightKey {
    request_id: WorkbenchRequestId,
    target: CloseTarget,
}

impl CloseFlightKey {
    pub(in crate::workbench_client) const fn new(
        request_id: WorkbenchRequestId,
        target: CloseTarget,
    ) -> Self {
        Self { request_id, target }
    }
    pub(in crate::workbench_client) const fn request_id(&self) -> &WorkbenchRequestId {
        &self.request_id
    }
    pub(in crate::workbench_client) const fn target(&self) -> &CloseTarget {
        &self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct CloseSelection {
    target: CloseTarget,
    label: WorkbenchLabel,
    binding: Option<NativeTerminalBinding>,
    attachment: Option<AttachmentKey>,
    base_revision: WorkbenchRevision,
}

impl CloseSelection {
    pub(in crate::workbench_client) fn new(
        target: CloseTarget,
        label: WorkbenchLabel,
        binding: impl Into<Option<NativeTerminalBinding>>,
        attachment: Option<AttachmentKey>,
        base_revision: WorkbenchRevision,
    ) -> Self {
        Self {
            target,
            label,
            binding: binding.into(),
            attachment,
            base_revision,
        }
    }
    pub(in crate::workbench_client) const fn target(&self) -> &CloseTarget {
        &self.target
    }
    pub(in crate::workbench_client) const fn base_revision(&self) -> WorkbenchRevision {
        self.base_revision
    }

    pub(in crate::workbench_client) const fn binding(&self) -> Option<&NativeTerminalBinding> {
        self.binding.as_ref()
    }

    pub(in crate::workbench_client) const fn attachment(&self) -> Option<&AttachmentKey> {
        self.attachment.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum CloseIntent {
    Close(CloseSelection),
    Set(Box<CloseSetCapture>),
    Resume {
        selection: CloseSelection,
        operation: WorkbenchRequestId,
        generation: WorkbenchTabCloseAttemptGeneration,
        status_token: CloseStatusToken,
    },
}

impl CloseIntent {
    pub(in crate::workbench_client) fn selection(&self) -> &CloseSelection {
        match self {
            Self::Close(selection) | Self::Resume { selection, .. } => selection,
            Self::Set(capture) => capture.first(),
        }
    }
    const fn kind(&self) -> CloseConfirmationKind {
        match self {
            Self::Close(_) | Self::Set(_) => CloseConfirmationKind::Close,
            Self::Resume { .. } => CloseConfirmationKind::Resume,
        }
    }
}

struct CloseConfirmation {
    token: CloseConfirmationToken,
    intent: CloseIntent,
    local_focus: Option<set::LocalFocus>,
    presentation: PresentationPhase,
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
pub(in crate::workbench_client) struct CloseStatusView<'a> {
    record: &'a CloseRecord,
    token: CloseStatusToken,
}

impl<'a> CloseStatusView<'a> {
    pub(in crate::workbench_client) const fn token(self) -> CloseStatusToken {
        self.token
    }
    pub(in crate::workbench_client) fn target_label(self) -> &'a str {
        self.record.selection.label.as_str()
    }
    pub(in crate::workbench_client) fn status(self) -> CloseStatusKind {
        self.record.kind()
    }
    pub(in crate::workbench_client) fn resumable(self) -> bool {
        self.record.resumable()
    }
    pub(in crate::workbench_client) fn observation_paused(self) -> bool {
        self.record.bounded_disposed_observation()
            && self.record.disposed_auto_admissions == DISPOSED_AUTO_ADMISSIONS
    }
    pub(in crate::workbench_client) const fn unavailable(self) -> bool {
        self.record.unavailable
    }
}

pub(in crate::workbench_client) struct CloseConfirmationView<'a> {
    confirmation: &'a CloseConfirmation,
    status: Option<CloseStatusView<'a>>,
    set_progress: Option<CloseSetStatus>,
}

impl<'a> CloseConfirmationView<'a> {
    pub(in crate::workbench_client) fn set(&self) -> Option<&'a CloseSetCapture> {
        match &self.confirmation.intent {
            CloseIntent::Set(capture) => Some(capture),
            _ => None,
        }
    }
    pub(in crate::workbench_client) const fn token(&self) -> CloseConfirmationToken {
        self.confirmation.token
    }
    pub(in crate::workbench_client) fn target_label(&self) -> &'a str {
        self.confirmation.intent.selection().label.as_str()
    }
    pub(in crate::workbench_client) const fn kind(&self) -> CloseConfirmationKind {
        self.confirmation.intent.kind()
    }
    pub(in crate::workbench_client) const fn interactive(&self) -> bool {
        self.confirmation.shown && self.confirmation.presentation.interactive()
    }
    pub(in crate::workbench_client) const fn presentation(&self) -> PresentationPhase {
        self.confirmation.presentation
    }
    pub(in crate::workbench_client) const fn resume_status(&self) -> Option<CloseStatusView<'a>> {
        self.status
    }
    pub(in crate::workbench_client) const fn resume_set_status(&self) -> Option<CloseSetStatus> {
        self.set_progress
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum CloseStateError {
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
    Client,
    StatusOnly,
    MembershipRecoveryPending(Box<CloseReconciliationOrigin>),
}

#[derive(Default)]
pub(in crate::workbench_client) struct CloseState {
    set: Option<set::CloseSetRun>,
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
    pub(super) fn permits_retired_membership(&self) -> bool {
        self.records.is_empty()
            && !self.set_active()
            && self.confirmation.is_none()
            && self.pending.is_none()
            && self.completed_handoff.is_none()
            && self.manual_pass.is_empty()
            && matches!(self.carrier, CloseCarrier::Client)
    }

    fn token(&mut self) -> Option<u64> {
        self.next_display_token = self.next_display_token.checked_add(1)?;
        Some(self.next_display_token)
    }

    fn record_index(&self, target: &CloseTarget) -> Option<usize> {
        self.records
            .iter()
            .position(|record| record.selection.target == *target)
    }

    fn capture_resume_permit(&self, target: &CloseTarget) -> Option<TabCloseResumePermit> {
        let record = &self.records[self.record_index(target)?];
        if !record.displayed || !record.resumable() {
            return None;
        }
        Some(TabCloseResumePermit {
            target: target.clone(),
            operation: record.operation.clone()?,
            generation: record.generation?,
            status_token: record.status_token?,
        })
    }

    fn accepts_resume_permit(&self, permit: &TabCloseResumePermit) -> bool {
        self.record_index(&permit.target).is_some_and(|index| {
            let record = &self.records[index];
            record.resumable()
                && record.operation.as_ref() == Some(&permit.operation)
                && record.generation == Some(permit.generation)
                && record.status_token == Some(permit.status_token)
        })
    }

    fn install_confirmation(&mut self, intent: CloseIntent) -> Result<(), CloseStateError> {
        let token = CloseConfirmationToken(self.token().ok_or(CloseStateError::Busy)?);
        self.confirmation = Some(CloseConfirmation {
            token,
            intent,
            local_focus: None,
            presentation: PresentationPhase::Pending,
            shown: false,
        });
        Ok(())
    }

    pub(in crate::workbench_client) fn open_confirmation(
        &mut self,
        selection: CloseSelection,
    ) -> Result<(), CloseStateError> {
        if self.confirmation.is_some() || self.pending.is_some() || self.set_active() {
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
        self.install_confirmation(intent)
    }

    fn open_resume_confirmation(
        &mut self,
        selection: CloseSelection,
        permit: &TabCloseResumePermit,
    ) -> Result<(), CloseStateError> {
        if self.confirmation.is_some() || self.pending.is_some() || self.set_active() {
            return Err(CloseStateError::Busy);
        }
        if selection.target != permit.target || !self.accepts_resume_permit(permit) {
            return Err(CloseStateError::NotResumable);
        }
        self.install_confirmation(CloseIntent::Resume {
            selection,
            operation: permit.operation.clone(),
            generation: permit.generation,
            status_token: permit.status_token,
        })
    }

    pub(in crate::workbench_client) fn cancel_confirmation(&mut self) {
        self.confirmation = None;
    }

    pub(in crate::workbench_client) fn confirmation(&self) -> Option<CloseConfirmationView<'_>> {
        let confirmation = self.confirmation.as_ref()?;
        let status = match &confirmation.intent {
            CloseIntent::Close(_) | CloseIntent::Set(_) => None,
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
            set_progress: status.and_then(|_| {
                self.set
                    .as_ref()?
                    .resume_status(confirmation.intent.selection().target())
            }),
        })
    }

    pub(in crate::workbench_client) fn status(
        &self,
        target: &CloseTarget,
    ) -> Option<CloseStatusView<'_>> {
        let record = &self.records[self.record_index(target)?];
        Some(CloseStatusView {
            token: record.status_token?,
            record,
        })
    }

    pub(in crate::workbench_client) fn latest_status(&self) -> Option<CloseStatusView<'_>> {
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

    pub(in crate::workbench_client) fn invalidate_display(&mut self) {
        for record in &mut self.records {
            record.displayed = false;
        }
        if let Some(confirmation) = &mut self.confirmation {
            confirmation.shown = false;
        }
    }

    pub(in crate::workbench_client) fn acknowledge_draw(
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
                    CloseIntent::Close(_) | CloseIntent::Set(_) => true,
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
            if confirmation.shown {
                confirmation.presentation.presented();
            }
        }
    }

    pub(in crate::workbench_client) fn confirmed_intent(
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

    pub(in crate::workbench_client) fn retain_submission(
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

    pub(in crate::workbench_client) fn record_acceptance(
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

    pub(in crate::workbench_client) fn observe_available(
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

    pub(in crate::workbench_client) fn charge_disposed_auto_admission(
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

    pub(in crate::workbench_client) fn observation_failed(&mut self, target: &CloseTarget) {
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

impl super::WorkbenchClientState {
    fn tab_close_resume_target(&self, tab_id: &WorkbenchTabId) -> Option<CloseTarget> {
        if !self.close_confirmation_scope_available()
            || self.close.set_active()
            || self.close.pending.is_some()
            || self.close.confirmation.is_some()
            || self.cache.tab_index(tab_id).is_none()
        {
            return None;
        }
        Some(CloseTarget::new(
            self.cache.site_id().clone(),
            self.cache.terminal_context_id()?.clone(),
            tab_id.clone(),
        ))
    }

    /// Mint a continuation only from an exact status that was actually visible.
    pub(in crate::workbench_client) fn capture_tab_close_resume_permit(
        &self,
        tab_id: &WorkbenchTabId,
    ) -> Option<TabCloseResumePermit> {
        let target = self.tab_close_resume_target(tab_id)?;
        self.close.capture_resume_permit(&target)
    }

    /// Revalidate a continuation retained by a covering child surface.
    ///
    /// Current display proof is intentionally not required here: opening the
    /// child revokes that proof. The exact record, resumable status and attempt
    /// still have to be current, and the resulting confirmation must earn a
    /// fresh complete draw before it can execute.
    pub(in crate::workbench_client) fn validate_tab_close_resume_permit(
        &self,
        tab_id: &WorkbenchTabId,
        permit: &TabCloseResumePermit,
    ) -> bool {
        self.tab_close_resume_target(tab_id)
            .is_some_and(|target| target == permit.target)
            && self.close.accepts_resume_permit(permit)
    }

    /// Recovery belongs to the exact Tab and an already presented status.
    /// The confirmation owner still validates the operation and attempt at admission.
    pub(in crate::workbench_client) fn tab_close_resume_available(
        &self,
        tab_id: &WorkbenchTabId,
    ) -> bool {
        self.capture_tab_close_resume_permit(tab_id).is_some()
    }

    pub(in crate::workbench_client) fn open_tab_close_resume(
        &mut self,
        tab_id: &WorkbenchTabId,
    ) -> Result<(), CloseStateError> {
        let permit = self
            .capture_tab_close_resume_permit(tab_id)
            .ok_or(CloseStateError::NotResumable)?;
        self.open_tab_close_resume_with_permit(tab_id, &permit)
    }

    pub(in crate::workbench_client) fn open_tab_close_resume_with_permit(
        &mut self,
        tab_id: &WorkbenchTabId,
        permit: &TabCloseResumePermit,
    ) -> Result<(), CloseStateError> {
        // This path has no ordinary-Close fallback: the covered continuation
        // must still identify the exact live resumable record.
        if !self.validate_tab_close_resume_permit(tab_id, permit) {
            return Err(CloseStateError::NotResumable);
        }
        let selection = self.close_set_selection(tab_id)?;
        let focus = self.close_set_focus();
        self.close.open_resume_confirmation(selection, permit)?;
        if let Some(confirmation) = &mut self.close.confirmation {
            confirmation.local_focus = Some(focus);
        }
        Ok(())
    }

    fn close_confirmation_scope_available(&self) -> bool {
        matches!(self.close.carrier, CloseCarrier::Client)
            && self.pending_refresh.is_none()
            && self.pending_membership.is_none()
            && !self.rename_draft_blocks_structure()
            && !self.mutation_blocks_structure()
            && self.panel_close.confirmation().is_none()
            && matches!(
                self.phase,
                super::ClientPhase::Ready(_)
                    | super::ClientPhase::SelectedWithoutAttachment(_)
                    | super::ClientPhase::RetryableAttachUnavailable(_)
            )
    }

    pub(in crate::workbench_client) fn open_close_confirmation(
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
        if !self.close_confirmation_scope_available() {
            return Err(CloseStateError::Busy);
        }
        // The footer's recovery shortcut addresses its presented operation, including a
        // halted background member, without selecting it or starting a different Close.
        let recovery = self
            .view()
            .close_status()
            .filter(|status| status.record.resumable())
            .map(|status| status.record.selection.target().tab_id().clone());
        if let Some(target) = recovery {
            return self.open_tab_close_resume(&target);
        }
        let selected = self.active().ok_or(CloseStateError::Busy)?;
        let tab = self
            .cache
            .tabs()
            .get(selected.index())
            .ok_or(CloseStateError::UnknownTarget)?;
        let panel = tab
            .panel(selected.panel_index())
            .ok_or(CloseStateError::UnknownTarget)?;
        let target = CloseTarget::new(
            self.cache.site_id().clone(),
            self.cache
                .terminal_context_id()
                .ok_or(CloseStateError::UnknownTarget)?
                .clone(),
            tab.tab_id().clone(),
        );
        let selection = CloseSelection::new(
            target,
            WorkbenchLabel::parse(&tab.display_label(selected.index()))
                .map_err(|_| CloseStateError::UnknownTarget)?,
            self.selected_presentation(panel).binding().cloned(),
            self.desired.as_ref().map(|key| key.anchor().clone()),
            self.cache.revision(),
        );
        self.close.open_confirmation(selection)
    }

    pub(in crate::workbench_client) fn cancel_close_confirmation(&mut self) {
        self.close.cancel_confirmation();
    }
    pub(in crate::workbench_client) fn invalidate_close_display(&mut self) {
        self.close.invalidate_display();
    }

    /// The same frozen premise governs UI retirement, planning and mailbox admission.
    /// Targeted recovery captures today's selection, not the older CloseRecord revision.
    fn close_confirmation_current(&self, confirmation: &CloseConfirmation) -> bool {
        if let Some(focus) = &confirmation.local_focus {
            let selection = confirmation.intent.selection();
            return matches!(confirmation.intent, CloseIntent::Resume { .. })
                && self.close_confirmation_scope_available()
                && self.close_set_focus() == *focus
                && self
                    .close_set_selection(selection.target().tab_id())
                    .as_ref()
                    == Ok(selection);
        }
        if let CloseIntent::Set(capture) = &confirmation.intent {
            return self.close_set_capture_current(capture);
        }
        if !matches!(self.close.carrier, CloseCarrier::Client) {
            return true;
        }
        let selection = confirmation.intent.selection();
        let Some(selected) = self.active() else {
            return false;
        };
        let Some(tab) = self.cache.tabs().get(selected.index()) else {
            return false;
        };
        let Some(panel) = tab.panel(selected.panel_index()) else {
            return false;
        };
        self.cache.site_id() == selection.target().site_id()
            && self.cache.terminal_context_id() == Some(selection.target().terminal_context_id())
            && self.cache.revision() == selection.base_revision()
            && tab.tab_id() == selection.target().tab_id()
            && self.selected_presentation(panel).binding() == selection.binding()
            && self.desired.as_ref().map(PaneKey::anchor) == selection.attachment()
    }

    /// Whether the armed confirmation's frozen facts no longer match live
    /// state. Display flags are deliberately excluded — a transient
    /// not-yet-redrawn state is recoverable, a drifted identity is not.
    fn close_confirmation_stale(&self) -> bool {
        let Some(confirmation) = &self.close.confirmation else {
            return false;
        };
        if let CloseIntent::Resume {
            selection,
            operation,
            generation,
            status_token,
        } = &confirmation.intent
        {
            let premise = self
                .close
                .record_index(&selection.target)
                .is_some_and(|index| {
                    let record = &self.close.records[index];
                    record.resumable()
                        && record.operation.as_ref() == Some(operation)
                        && record.generation == Some(*generation)
                        && record.status_token == Some(*status_token)
                });
            if !premise {
                return true;
            }
        }
        !self.close_confirmation_current(confirmation)
    }

    /// Settle armed close-family confirmations whose frozen premises no longer
    /// match live state: cancel them and post a bounded localized notice.
    /// Runs after every applied mutation and before any captured key is
    /// decided, so a stale capture never lingers armed swallowing input.
    pub(in crate::workbench_client) fn settle_stale_close_confirmations(&mut self) -> bool {
        let mut settled = false;
        if self.close_confirmation_stale() {
            self.close.cancel_confirmation();
            self.set_notice(ClientNotice::ConfirmationCancelled);
            settled = true;
        }
        if self.armed_panel_close_stale() {
            self.cancel_panel_close_confirmation();
            self.set_notice(ClientNotice::ConfirmationCancelled);
            settled = true;
        }
        settled
    }

    /// Settles a geometry-blocked draw without conflating temporary suspension
    /// with cancellation. A new prompt is refused; an admitted prompt keeps its
    /// frozen target and can be presented again with the same token.
    pub(in crate::workbench_client) fn settle_close_confirmation_not_presented(
        &mut self,
    ) -> Option<GeometrySettlement> {
        let confirmation = self.close.confirmation.as_mut()?;
        confirmation.shown = false;
        let settlement = confirmation.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.close.cancel_confirmation();
        }
        Some(settlement)
    }

    pub(in crate::workbench_client) fn acknowledge_close_draw(
        &mut self,
        prompt: Option<CloseConfirmationToken>,
        status: Option<CloseStatusToken>,
    ) {
        self.close.acknowledge_draw(prompt, status);
    }
}

impl<'a> super::WorkbenchClientView<'a> {
    pub(crate) fn close_set_status(&self) -> Option<CloseSetStatus> {
        self.state.close_set_display_status()
    }
    pub(in crate::workbench_client) fn close_confirmation(
        &self,
    ) -> Option<CloseConfirmationView<'a>> {
        self.state.close.confirmation()
    }
    pub(in crate::workbench_client) fn close_status(&self) -> Option<CloseStatusView<'a>> {
        if let Some(status) = self
            .state
            .close
            .confirmation()
            .and_then(|view| view.resume_status())
        {
            return Some(status);
        }
        if let Some(status) = self
            .state
            .close
            .set
            .as_ref()
            .and_then(set::CloseSetRun::halted_target)
            .and_then(|target| self.state.close.status(target))
        {
            return Some(status);
        }
        if let Some(selected) = self.state.active() {
            let target = CloseTarget::new(
                self.state.cache.site_id().clone(),
                self.state.cache.terminal_context_id()?.clone(),
                selected.key().anchor().tab_id().clone(),
            );
            if let Some(status) = self.state.close.status(&target) {
                return Some(status);
            }
        }
        self.state.close.latest_status()
    }
}
