//! Owns Client close confirmation presentation, retained status records, and exact-generation
//! resume permits across Tab, Panel and Workspace closure. It observes Server close authority
//! without deciding physical disposal or implicitly replaying an unknown close.

use super::{AttachmentKey, ClientNotice, PaneKey};
use crate::exchange_values::workbench::{
    WorkbenchTabCloseAccepted, panel_close::WorkbenchPanelCloseAccepted,
};
use crate::localization::CloseImpact;
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};
use crate::workbench_projection::ClientTerminalBinding;
use orchbee_workbench_protocol::{
    SessionId, WorkbenchLabel, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabClosePublicationReason,
    WorkbenchTabCloseStatus, WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::time::{Duration, Instant};

mod flight;
mod reconciliation;
mod set;
#[cfg(test)]
pub(crate) use flight::CloseReadMode;
pub(crate) use flight::{
    CloseOperation, CloseOperationKind, CloseResult, CloseWorkerReturn, PendingCloseResult,
};
pub(crate) use reconciliation::CloseLeaseAcknowledgement;
pub(super) use reconciliation::PreparedCloseConsumption;
pub(crate) use reconciliation::{
    CloseReconciliationMarker, CloseReconciliationOrigin, DefinitiveClosedAttachment,
};
pub(crate) use set::CloseSetCapture;
pub(crate) use set::CloseSetStatus;
pub(crate) use set::{CloseSetKind, close_set_targets};

const MAXIMUM_CLOSE_RECORDS: usize = 256;
const DISPOSED_AUTO_ADMISSIONS: u8 = 32;
/// The Tab leaving its strip reports an ordinary close, so a progressing close stays off the status
/// line this long after its submission. A real PowerShell Tab closes within about 0.15 s; this
/// leaves room for a slower shell, so the indication appears only for a close that is really slow,
/// such as one waiting out the Server's five-second grace.
const CLOSE_PENDING_DELAY: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseConfirmationToken(u64);

impl CloseConfirmationToken {
    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CloseStatusToken(u64);

pub(crate) use crate::localization::CloseConfirmationKind;

pub(crate) use crate::localization::CloseStatusKind;

/// What one retained close ends: a whole Tab, or one Panel of several in a Tab's split.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CloseTarget {
    session_id: SessionId,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: Option<WorkbenchPanelId>,
}

impl CloseTarget {
    pub(crate) const fn new(
        session_id: SessionId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
    ) -> Self {
        Self {
            session_id,
            terminal_context_id,
            tab_id,
            panel_id: None,
        }
    }

    /// The close of one Panel that leaves its Tab's split, not the Tab.
    pub(crate) const fn panel(
        session_id: SessionId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
    ) -> Self {
        Self {
            session_id,
            terminal_context_id,
            tab_id,
            panel_id: Some(panel_id),
        }
    }

    pub(crate) const fn session_id(&self) -> &SessionId {
        &self.session_id
    }
    pub(crate) const fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }
    pub(crate) const fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }
    pub(crate) const fn panel_id(&self) -> Option<&WorkbenchPanelId> {
        self.panel_id.as_ref()
    }
}

/// Exact authority to continue one previously presented, resumable Close.
///
/// Only the close owner can mint this value, and only while the matching
/// status is genuinely displayed. A covering child may retain it, but every
/// use revalidates the live target, operation, attempt and status identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TabCloseResumePermit {
    target: CloseTarget,
    operation: WorkbenchRequestId,
    generation: WorkbenchTabCloseAttemptGeneration,
    status_token: CloseStatusToken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CloseFlightKey {
    request_id: WorkbenchRequestId,
    target: CloseTarget,
}

impl CloseFlightKey {
    pub(crate) const fn new(request_id: WorkbenchRequestId, target: CloseTarget) -> Self {
        Self { request_id, target }
    }
    pub(crate) const fn request_id(&self) -> &WorkbenchRequestId {
        &self.request_id
    }
    pub(crate) const fn target(&self) -> &CloseTarget {
        &self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CloseSelection {
    target: CloseTarget,
    label: WorkbenchLabel,
    binding: Option<ClientTerminalBinding>,
    attachment: Option<AttachmentKey>,
    base_revision: WorkbenchRevision,
}

impl CloseSelection {
    pub(crate) fn new(
        target: CloseTarget,
        label: WorkbenchLabel,
        binding: impl Into<Option<ClientTerminalBinding>>,
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
    pub(crate) const fn target(&self) -> &CloseTarget {
        &self.target
    }
    pub(crate) const fn base_revision(&self) -> WorkbenchRevision {
        self.base_revision
    }

    pub(crate) const fn binding(&self) -> Option<&ClientTerminalBinding> {
        self.binding.as_ref()
    }

    pub(crate) const fn attachment(&self) -> Option<&AttachmentKey> {
        self.attachment.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CloseIntent {
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
    pub(crate) fn selection(&self) -> &CloseSelection {
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
    submitted_at: Instant,
    /// The pending delay passed, or this record already reached the status line. It stays there,
    /// so a recovered read never turns a reported close silent again.
    announced: bool,
    initial_read_pending: bool,
    reconciliation: Option<reconciliation::RetainedReconciliation>,
}

impl CloseRecord {
    fn new(selection: CloseSelection, submitted_at: Instant) -> Self {
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
            submitted_at,
            announced: false,
            initial_read_pending: false,
            reconciliation: None,
        }
    }

    /// Whether the status line reports this close: every failure, unknown and recovery at once,
    /// and a close still progressing normally only after the pending delay.
    fn announced(&self) -> bool {
        self.announced || self.unavailable || self.uncertain || !self.progressing()
    }

    /// Whether the close is on its ordinary way out: submitted, accepted, closing or closed, with
    /// no failure, unknown outcome or lost observation.
    fn progressing(&self) -> bool {
        match &self.status {
            None
            | Some(
                WorkbenchTabCloseStatus::Closing { .. } | WorkbenchTabCloseStatus::Closed { .. },
            ) => true,
            Some(WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. }) => {
                self.bounded_disposed_observation()
            }
            Some(
                WorkbenchTabCloseStatus::CleanupBlocked { .. }
                | WorkbenchTabCloseStatus::OutcomeUnknown { .. },
            ) => false,
        }
    }

    /// Whether the close stopped on its way: it failed, its outcome is unknown, or its observation
    /// was lost. What it removes shows again with its report.
    fn stopped(&self) -> bool {
        self.unavailable || self.uncertain || !self.progressing()
    }

    fn leaving(&self, tab_id: &WorkbenchTabId) -> bool {
        self.selection.target.panel_id.is_none()
            && self.selection.target.tab_id() == tab_id
            && !self.stopped()
    }

    /// A Panel close offers no Resume: as in the GUI, a stop is reported and leaves the Panel in
    /// place.
    fn resumable(&self) -> bool {
        self.selection.target.panel_id.is_none()
            && !self.unavailable
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
pub(crate) struct CloseStatusView<'a> {
    record: &'a CloseRecord,
    token: CloseStatusToken,
}

impl<'a> CloseStatusView<'a> {
    pub(crate) const fn token(self) -> CloseStatusToken {
        self.token
    }
    pub(crate) fn target_label(self) -> &'a str {
        self.record.selection.label.as_str()
    }
    pub(crate) fn target_tab_id(self) -> &'a WorkbenchTabId {
        &self.record.selection.target.tab_id
    }
    pub(crate) fn status(self) -> CloseStatusKind {
        self.record.kind()
    }
    pub(crate) fn resumable(self) -> bool {
        self.record.resumable()
    }
    pub(crate) fn observation_paused(self) -> bool {
        self.record.bounded_disposed_observation()
            && self.record.disposed_auto_admissions == DISPOSED_AUTO_ADMISSIONS
    }
    pub(crate) const fn unavailable(self) -> bool {
        self.record.unavailable
    }
    /// Whether this close ends one Panel of a split, which the status names instead of a Tab.
    pub(crate) const fn panel(self) -> bool {
        self.record.selection.target.panel_id.is_some()
    }
}

pub(crate) struct CloseConfirmationView<'a> {
    confirmation: &'a CloseConfirmation,
    status: Option<CloseStatusView<'a>>,
    set_progress: Option<CloseSetStatus>,
}

impl<'a> CloseConfirmationView<'a> {
    pub(crate) fn set(&self) -> Option<&'a CloseSetCapture> {
        match &self.confirmation.intent {
            CloseIntent::Set(capture) => Some(capture),
            _ => None,
        }
    }
    pub(crate) const fn token(&self) -> CloseConfirmationToken {
        self.confirmation.token
    }
    /// The exact Tabs this confirmation closes.
    fn tab_ids(&self) -> &'a [WorkbenchTabId] {
        match &self.confirmation.intent {
            CloseIntent::Set(capture) => capture.tabs(),
            CloseIntent::Close(selection) | CloseIntent::Resume { selection, .. } => {
                std::slice::from_ref(&selection.target.tab_id)
            }
        }
    }
    pub(crate) fn target_label(&self) -> &'a str {
        self.confirmation.intent.selection().label.as_str()
    }
    pub(crate) fn target_tab_id(&self) -> &'a WorkbenchTabId {
        &self.confirmation.intent.selection().target.tab_id
    }
    pub(crate) const fn kind(&self) -> CloseConfirmationKind {
        self.confirmation.intent.kind()
    }
    pub(crate) const fn interactive(&self) -> bool {
        self.confirmation.shown && self.confirmation.presentation.interactive()
    }
    pub(crate) const fn presentation(&self) -> PresentationPhase {
        self.confirmation.presentation
    }
    pub(crate) const fn resume_status(&self) -> Option<CloseStatusView<'a>> {
        self.status
    }
    pub(crate) const fn resume_set_status(&self) -> Option<CloseSetStatus> {
        self.set_progress
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseStateError {
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
pub(crate) struct CloseState {
    set: Option<set::TabSetFollow>,
    records: Vec<CloseRecord>,
    confirmation: Option<CloseConfirmation>,
    next_display_token: u64,
    pending: Option<CloseOperation>,
    completed_handoff: Option<PendingCloseResult>,
    last_read_admission: Option<std::time::Instant>,
    /// A Workbench publication arrived while a close was retained. The Server publishes a closed
    /// Tab as it finishes, so the next status read goes out without waiting for `READ_INTERVAL`.
    publication_wake: bool,
    observation_cursor: usize,
    manual_pass: std::collections::VecDeque<CloseTarget>,
    carrier: CloseCarrier,
}

impl CloseState {
    /// No close work is open. Retained Panel closes between their reads are none, however many:
    /// each follows its own leaf, which a membership that no longer draws that leaf ends, so none
    /// holds navigation or structure.
    pub(super) fn permits_retired_membership(&self) -> bool {
        self.records
            .iter()
            .all(|record| record.selection.target.panel_id.is_some())
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

    /// Whether this Tab has already left the Client's view: its close is progressing normally, or
    /// it is a member of a set the Server is closing, so the strip and navigation leave it out at
    /// once instead of when the Server's publication lands. A close that fails, turns unknown or
    /// loses its observation, and a set member left open, shows the Tab again with its report.
    pub(crate) fn tab_leaving(&self, tab_id: &WorkbenchTabId) -> bool {
        self.set_leaving(tab_id) || self.records.iter().any(|record| record.leaving(tab_id))
    }

    /// Whether the Server accepted the close of this leaving Tab, so the selection may move off it.
    pub(crate) fn tab_close_accepted(&self, tab_id: &WorkbenchTabId) -> bool {
        self.set_leaving(tab_id)
            || self
                .records
                .iter()
                .any(|record| record.leaving(tab_id) && record.operation.is_some())
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

    pub(crate) fn open_confirmation(
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

    pub(crate) fn cancel_confirmation(&mut self) {
        self.confirmation = None;
    }

    pub(crate) fn confirmation(&self) -> Option<CloseConfirmationView<'_>> {
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
            set_progress: status
                .and_then(|_| self.set_resume_status(confirmation.intent.selection().target())),
        })
    }

    pub(crate) fn status(&self, target: &CloseTarget) -> Option<CloseStatusView<'_>> {
        let record = &self.records[self.record_index(target)?];
        Some(CloseStatusView {
            token: record.status_token?,
            record,
        })
    }

    fn latest_status(&self, admit: impl Fn(&CloseRecord) -> bool) -> Option<CloseStatusView<'_>> {
        self.records
            .iter()
            .filter(|record| admit(record))
            .filter_map(|record| {
                Some(CloseStatusView {
                    record,
                    token: record.status_token?,
                })
            })
            .max_by_key(|status| status.token.0)
    }

    pub(crate) fn invalidate_display(&mut self) {
        for record in &mut self.records {
            record.displayed = false;
        }
        if let Some(confirmation) = &mut self.confirmation {
            confirmation.shown = false;
        }
    }

    pub(crate) fn acknowledge_draw(
        &mut self,
        prompt: Option<CloseConfirmationToken>,
        status: Option<CloseStatusToken>,
    ) {
        for record in &mut self.records {
            record.displayed = status.is_some() && record.status_token == status;
            record.announced |= record.displayed;
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

    pub(crate) fn confirmed_intent(
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

    pub(crate) fn retain_submission(
        &mut self,
        selection: CloseSelection,
        submitted_at: Instant,
    ) -> Result<(), CloseStateError> {
        if self.record_index(&selection.target).is_some() {
            return Ok(());
        }
        if self.records.len() == MAXIMUM_CLOSE_RECORDS {
            return Err(CloseStateError::Busy);
        }
        self.records.push(CloseRecord::new(selection, submitted_at));
        Ok(())
    }

    /// Retains a close the Server accepted from a request this Client sent as a Panel action, and
    /// follows it from its first status read as a submitted Tab close is followed: the close of a
    /// Tab's last Panel as the close of that Tab, any other as the close of its Panel. A target that
    /// already holds a retained close keeps that record.
    fn retain_accepted(
        &mut self,
        selection: CloseSelection,
        accepted: &WorkbenchTabCloseAccepted,
        submitted_at: Instant,
    ) -> Result<(), CloseStateError> {
        if self.record_index(&selection.target).is_some()
            || self.records.len() == MAXIMUM_CLOSE_RECORDS
        {
            return Err(CloseStateError::Busy);
        }
        let target = selection.target.clone();
        self.records.push(CloseRecord::new(selection, submitted_at));
        if let Err(error) = self.record_acceptance(&target, accepted) {
            self.records.pop();
            return Err(error);
        }
        if let Some(record) = self.records.last_mut() {
            record.initial_read_pending = true;
        }
        Ok(())
    }

    pub(crate) fn record_acceptance(
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

    pub(crate) fn observe_available(
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
            WorkbenchTabCloseStatus::Closing { .. }
            | WorkbenchTabCloseStatus::CleanupBlocked { .. }
            | WorkbenchTabCloseStatus::OutcomeUnknown { .. }
            | WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. } => None,
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

    /// Whether the retained close of this Panel stopped; `None` when none is retained.
    pub(super) fn panel_close_stopped(&self, target: &CloseTarget) -> Option<bool> {
        Some(self.records[self.record_index(target)?].stopped())
    }

    /// Ends the follow of every Panel close whose Panel `drawn` no longer holds: the publication
    /// that removed it settled the close. A read in flight keeps its record until it settles.
    pub(super) fn prune_departed_panel_closes(&mut self, drawn: impl Fn(&CloseTarget) -> bool) {
        let pending = self
            .pending
            .as_ref()
            .map(|operation| operation.key().target().clone());
        let departed = |target: &CloseTarget| {
            target.panel_id.is_some() && !drawn(target) && pending.as_ref() != Some(target)
        };
        self.records
            .retain(|record| !departed(&record.selection.target));
        self.manual_pass.retain(|target| !departed(target));
    }

    pub(crate) fn charge_disposed_auto_admission(
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

    pub(crate) fn observation_failed(&mut self, target: &CloseTarget) {
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
            self.cache.session_id().clone(),
            self.cache.terminal_context_id()?.clone(),
            tab_id.clone(),
        ))
    }

    /// Mint a continuation only from an exact status that was actually visible.
    pub(crate) fn capture_tab_close_resume_permit(
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
    pub(crate) fn validate_tab_close_resume_permit(
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
    pub(crate) fn tab_close_resume_available(&self, tab_id: &WorkbenchTabId) -> bool {
        self.capture_tab_close_resume_permit(tab_id).is_some()
    }

    pub(crate) fn open_tab_close_resume(
        &mut self,
        tab_id: &WorkbenchTabId,
    ) -> Result<(), CloseStateError> {
        let permit = self
            .capture_tab_close_resume_permit(tab_id)
            .ok_or(CloseStateError::NotResumable)?;
        self.open_tab_close_resume_with_permit(tab_id, &permit)
    }

    pub(crate) fn open_tab_close_resume_with_permit(
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

    /// Follows the accepted close of a Tab's last Panel as the close of that Tab, which it is on the
    /// Server: from this acceptance the Tab leaves the Client's view, the selection follows it, and
    /// its status reads, report and Resume are those of any accepted Tab close. Refused while the
    /// Tab already holds a retained close, which keeps its own record and report.
    pub(super) fn retain_last_panel_close(
        &mut self,
        accepted: &WorkbenchTabCloseAccepted,
        submitted_at: Instant,
    ) -> Result<(), CloseStateError> {
        if !matches!(self.close.carrier, CloseCarrier::Client) {
            return Err(CloseStateError::Busy);
        }
        let selection = self.close_set_selection(&accepted.tab_id)?;
        self.close
            .retain_accepted(selection, accepted, submitted_at)
    }

    /// Follows the accepted close of one Panel of several as the close of that Panel, by its
    /// operation: a read that sees it stop shows the Panel again with its report, and a completed
    /// close or the publication removing the Panel ends the follow. Refused while the Panel already
    /// holds a retained close, which keeps its own record and report.
    pub(super) fn retain_panel_close(
        &mut self,
        accepted: &WorkbenchPanelCloseAccepted,
        submitted_at: Instant,
    ) -> Result<(), CloseStateError> {
        if !matches!(self.close.carrier, CloseCarrier::Client) {
            return Err(CloseStateError::Busy);
        }
        let tab = self
            .cache
            .tab_index(&accepted.tab_id)
            .and_then(|index| self.cache.tabs().get(index))
            .ok_or(CloseStateError::UnknownTarget)?;
        let panel = tab
            .panel_index(&accepted.panel_id)
            .and_then(|index| tab.panel(index))
            .ok_or(CloseStateError::UnknownTarget)?;
        let presentation = self.selected_presentation(panel);
        let selection = CloseSelection::new(
            CloseTarget::panel(
                self.cache.session_id().clone(),
                accepted.terminal_context_id.clone(),
                accepted.tab_id.clone(),
                accepted.panel_id.clone(),
            ),
            WorkbenchLabel::parse(presentation.display_label())
                .map_err(|_| CloseStateError::UnknownTarget)?,
            presentation.binding().cloned(),
            None,
            self.cache.revision(),
        );
        self.close.retain_accepted(
            selection,
            &WorkbenchTabCloseAccepted {
                close_request_id: accepted.close_request_id.clone(),
                terminal_context_id: accepted.terminal_context_id.clone(),
                tab_id: accepted.tab_id.clone(),
                accepted_revision: accepted.accepted_revision,
            },
            submitted_at,
        )
    }

    /// Ends the follow of each Panel close whose Panel the drawn membership no longer holds, and
    /// shows each leaving Panel again once its own close stopped. Runs after every settled close
    /// step.
    pub(super) fn settle_panel_close_follow(&mut self) {
        let cache = &self.cache;
        self.close
            .prune_departed_panel_closes(|target| panel_drawn(cache, target));
        self.reveal_stopped_panel_closes();
    }

    /// Whether this Tab has left the Client's view: its own close is progressing, alone or in a
    /// set, or every Panel it holds is leaving with its accepted close.
    pub(crate) fn tab_leaving(&self, tab_id: &WorkbenchTabId) -> bool {
        self.close.tab_leaving(tab_id) || self.every_panel_leaving(tab_id)
    }

    /// Whether the Tab at `index` is still in the Client's view, not leaving with its close.
    pub(crate) fn tab_shown(&self, index: usize) -> bool {
        self.cache
            .tabs()
            .get(index)
            .is_some_and(|tab| !self.tab_leaving(tab.tab_id()))
    }

    /// Where the selection goes once the Server accepts the selected Tab's close, alone or in a
    /// set, or the close of every Panel it holds: the next Tab still shown, else the previous one,
    /// the survivor the removal itself would select. The Panel then leaves at the next Tab's first
    /// frame instead of when the publication lands. There is none when every Tab leaves; the last
    /// removal then empties the Client. A set member left open, or a Tab one of whose Panel closes
    /// stopped, comes back unselected.
    pub(crate) fn leaving_selection_successor(&self) -> Option<usize> {
        let selected = self.active()?.index();
        let tab_id = self.cache.tabs().get(selected)?.tab_id();
        if !self.close.tab_close_accepted(tab_id) && !self.every_panel_leaving(tab_id) {
            return None;
        }
        (selected + 1..self.cache.tabs().len())
            .find(|index| self.tab_shown(*index))
            .or_else(|| (0..selected).rev().find(|index| self.tab_shown(*index)))
    }

    /// Whether the Server accepted the close of the selected Tab itself. Closing its last Panel
    /// alone does not count, so that close keeps the post-close landing.
    pub(crate) fn selected_tab_close_accepted(&self) -> bool {
        self.active()
            .and_then(|active| self.cache.tabs().get(active.index()))
            .is_some_and(|tab| self.close.tab_close_accepted(tab.tab_id()))
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

    pub(crate) fn open_close_confirmation(&mut self) -> Result<(), CloseStateError> {
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
            self.cache.session_id().clone(),
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

    pub(crate) fn cancel_close_confirmation(&mut self) {
        self.close.cancel_confirmation();
    }

    pub(crate) fn invalidate_close_display(&mut self) {
        self.close.invalidate_display();
    }

    /// Announces each close still progressing at the end of its pending delay.
    pub(crate) fn settle_close_pending(&mut self, now: Instant) -> bool {
        let mut changed = false;
        for record in &mut self.close.records {
            if !record.announced() && now >= record.submitted_at + CLOSE_PENDING_DELAY {
                record.announced = true;
                changed = true;
            }
        }
        changed
    }

    pub(crate) fn close_pending_deadline(&self) -> Option<Instant> {
        self.close
            .records
            .iter()
            .filter(|record| !record.announced())
            .map(|record| record.submitted_at + CLOSE_PENDING_DELAY)
            .min()
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
        self.cache.session_id() == selection.target().session_id()
            && self.cache.terminal_context_id() == Some(selection.target().terminal_context_id())
            && self.cache.revision() == selection.base_revision()
            && tab.tab_id() == selection.target().tab_id()
            && self.selected_presentation(panel).binding() == selection.binding()
            && self.desired.as_ref().map(PaneKey::anchor) == selection.attachment()
    }

    /// Whether the armed confirmation's frozen facts no longer match live
    /// state. Display flags are deliberately excluded — a transient
    /// not-yet-redrawn state is recoverable, a drifted identity is not.
    /// While the Server decides an if-idle set, the set owns its hidden
    /// confirmation: the Server's answer, or a resend's own check, settles it.
    fn close_confirmation_stale(&self) -> bool {
        let Some(confirmation) = &self.close.confirmation else {
            return false;
        };
        if self.close.set_deciding() {
            return false;
        }
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
    pub(crate) fn settle_stale_close_confirmations(&mut self) -> bool {
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
    pub(crate) fn settle_close_confirmation_not_presented(&mut self) -> Option<GeometrySettlement> {
        let confirmation = self.close.confirmation.as_mut()?;
        confirmation.shown = false;
        let settlement = confirmation.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.close.cancel_confirmation();
            self.set_notice(ClientNotice::DialogRefused);
        }
        Some(settlement)
    }

    pub(crate) fn acknowledge_close_draw(
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
    pub(crate) fn close_confirmation(&self) -> Option<CloseConfirmationView<'a>> {
        self.state.close.confirmation()
    }
    /// Whether the Server is deciding if the armed Close needs asking; its confirmation stays
    /// hidden until the answer.
    pub(crate) fn close_deciding(&self) -> bool {
        self.state.close.set_deciding()
            || self.state.close.pending.as_ref().is_some_and(|operation| {
                matches!(operation.kind(), CloseOperationKind::CloseIfIdle { .. })
            })
    }
    /// What `confirmation` ends, from the attachments `settled` reports as Host-settled. A Resume
    /// whose Server status proved disposal has no running terminal left.
    pub(crate) fn close_impact(
        &self,
        confirmation: &CloseConfirmationView<'a>,
        settled: &dyn Fn(&super::PaneRef) -> bool,
    ) -> CloseImpact {
        let impact = self
            .state
            .tabs_close_impact(confirmation.tab_ids(), settled);
        if confirmation
            .resume_status()
            .is_some_and(|status| status.record.disposed)
        {
            return CloseImpact {
                running: 0,
                ended: impact.running + impact.ended,
            };
        }
        impact
    }
    /// The close a recovery or a Client control request addresses, reported or not.
    pub(crate) fn close_status(&self) -> Option<CloseStatusView<'a>> {
        self.select_close_status(|_| true)
    }
    /// The close the status line reports now.
    pub(crate) fn announced_close_status(&self) -> Option<CloseStatusView<'a>> {
        self.select_close_status(CloseRecord::announced)
    }
    /// Whether the selected Tab, or its focused Panel, is closing unreported, so its terminal
    /// ending is no news.
    pub(crate) fn close_quiet_on_selected(&self) -> bool {
        let Some(selected) = self.state.active() else {
            return false;
        };
        let focus = self.state.focus_panel_id();
        self.state.close.records.iter().any(|record| {
            let target = &record.selection.target;
            target.tab_id() == selected.key().anchor().tab_id()
                && target
                    .panel_id
                    .as_ref()
                    .is_none_or(|panel| Some(panel) == focus)
                && !record.announced()
        })
    }
    fn select_close_status(
        &self,
        admit: impl Fn(&CloseRecord) -> bool,
    ) -> Option<CloseStatusView<'a>> {
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
            .and_then(set::TabSetFollow::halted_target)
            .and_then(|target| self.state.close.status(target))
        {
            return Some(status);
        }
        if let Some(selected) = self.state.active() {
            let target = CloseTarget::new(
                self.state.cache.session_id().clone(),
                self.state.cache.terminal_context_id()?.clone(),
                selected.key().anchor().tab_id().clone(),
            );
            if let Some(status) = self
                .state
                .close
                .status(&target)
                .filter(|status| admit(status.record))
            {
                return Some(status);
            }
        }
        self.state.close.latest_status(admit)
    }
}

/// Whether `tabs` draws the Panel that the Panel close `target` names, in its Session and Context.
pub(super) fn panel_drawn(
    tabs: &crate::workbench_projection::ClientContextTabs,
    target: &CloseTarget,
) -> bool {
    tabs.session_id() == target.session_id()
        && tabs.terminal_context_id() == Some(target.terminal_context_id())
        && target.panel_id().is_some_and(|panel| {
            tabs.tab_index(target.tab_id())
                .and_then(|index| tabs.tabs().get(index))
                .is_some_and(|tab| tab.panel_index(panel).is_some())
        })
}
