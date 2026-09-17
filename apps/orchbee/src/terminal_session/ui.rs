use super::{
    mailbox::{
        AdmissionError, CausalSettlement, CommandMailbox, CreateMarker, CreatePrecommitOutcome,
        EventError, MoveAdmissionError, MutationResult, QueuedCommand, RefreshSettlement,
        SwitchMarker, Tagged, UiReply, WorkerEvent,
    },
    state::{
        AttachmentKey, CarrierFailure, ControllerMode, CreateOperation, CreateOperationKey,
        CreateProjectionDecision, CreateTerminalProvenance, FrameIdentity, NativeSessionState,
        NativeSessionView as StateSessionView, NativeTabView as StateTabView,
        NavigatorRefreshStatus, OperationDeadlines, PanelRequestKind, PendingCreateCommit,
        PendingMoveCommit, PendingPanelCommit, RefreshDecision, SelectionPlan, SessionNotice,
        SessionPhaseKind, SetupEventDisposition,
    },
};
use crate::{
    client_view::{ClientAction, PanelDirection, SessionActionDisposition},
    local_client::{
        LocalOperationClock,
        workbench::{WorkbenchTabCreatePrecommit, WorkbenchTabCreateRequest},
    },
    terminal_projection::TerminalProjection,
};
use orchbee_client_server_protocol::{ControlInput, Geometry, TerminalViewerAction};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchLabel, WorkbenchTabMoveDirection,
    WorkbenchTerminalGeometry,
};
use std::sync::Arc;

mod close_reconciliation;
mod membership;
mod move_membership;
mod panel;
#[cfg(target_os = "linux")]
mod retired_membership;
mod tab_close;
mod workspace_setup;
pub(crate) use workspace_setup::{
    NativeLocationLabelStatus, NativeLocationLabelTarget, NativeSetupDraftView, NativeSetupEdit,
    NativeSetupInputContext, NativeSetupStage, NativeSetupStatus, NativeSetupToken,
};
mod tab_rename;
pub(crate) use tab_close::{
    NativeCloseConfirmationKind, NativeCloseConfirmationToken, NativeCloseConfirmationView,
    NativeCloseStatusKind, NativeCloseStatusToken, NativeCloseStatusView, NativeDrawReceipt,
};
pub(crate) use tab_rename::{
    NativeRenameDraftToken, NativeRenameDraftView, NativeRenameEdit, NativeRenameInputContext,
};
mod create_membership;
mod switch_membership;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeDisplayPhase {
    Starting,
    Empty,
    Ready,
    Creating,
    CreateCommittedNeedDetach,
    ProjectionUnavailable,
    MoveProjectionUnavailable,
    PanelProjectionUnavailable,
    RenameProjectionUnavailable,
    Switching,
    SelectedWithoutAttachment,
    RetryableAttachUnavailable,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeDisplayController {
    Controlling,
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeDisplayProvenance {
    Started,
    Pending,
    Unavailable,
    Failed,
    OutcomeUnknown,
    BindingOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSessionNotice {
    LocalBusy,
    NoTabAtPosition(u8),
    CreateNotCreated,
    MoveNotMoved,
    PanelNotApplied,
    RenameNotRenamed,
    CloseRequestNotApplied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeRefreshStatus {
    Refreshing,
    Success(u64),
    Failure(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeProjectionStatus {
    Empty,
    FrameAvailable,
    WaitingForFullFrame,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeDisplayRetirement {
    Unavailable,
    OutcomeUnknown,
}

#[derive(Clone, Copy)]
pub(crate) struct NativeTabView<'a> {
    inner: StateTabView<'a>,
}

impl<'a> NativeTabView<'a> {
    pub(crate) const fn position(&self) -> usize {
        self.inner.position()
    }

    pub(crate) const fn active(&self) -> bool {
        self.inner.active()
    }

    pub(crate) fn tab_label(&self) -> &'a str {
        self.inner.tab_label()
    }

    pub(crate) fn panel_count(&self) -> usize {
        self.inner.panel_count()
    }

    pub(crate) fn panel_tree(&self) -> &'a super::NativePanelTree {
        self.inner.panel_tree()
    }

    pub(crate) fn active_panel(&self) -> Option<usize> {
        self.inner.active_panel()
    }

    pub(crate) fn panel(&self, index: usize) -> Option<&'a super::NativePanel> {
        self.inner.panel(index)
    }
}

pub(crate) struct NativeSessionView<'a> {
    state: Option<StateSessionView<'a>>,
    projection: Option<&'a TerminalProjection>,
    projection_status: NativeProjectionStatus,
    retirement: Option<NativeDisplayRetirement>,
}

impl<'a> NativeSessionView<'a> {
    pub(crate) fn context_count(&self) -> usize {
        self.state
            .as_ref()
            .map_or(0, StateSessionView::context_count)
    }

    pub(crate) fn active_context_position(&self) -> Option<usize> {
        self.state.as_ref()?.active_context_position()
    }

    pub(crate) fn context_at(&self, index: usize) -> Option<super::state::NativeContextView<'a>> {
        self.state.as_ref()?.context_at(index)
    }

    pub(crate) const fn phase(&self) -> NativeDisplayPhase {
        let Some(state) = self.state.as_ref() else {
            return NativeDisplayPhase::Starting;
        };
        match state.phase() {
            SessionPhaseKind::Empty => NativeDisplayPhase::Empty,
            SessionPhaseKind::Starting => NativeDisplayPhase::Starting,
            SessionPhaseKind::Ready => NativeDisplayPhase::Ready,
            SessionPhaseKind::Creating => NativeDisplayPhase::Creating,
            SessionPhaseKind::CreateCommittedNeedDetach => {
                NativeDisplayPhase::CreateCommittedNeedDetach
            }
            SessionPhaseKind::ProjectionUnavailable => NativeDisplayPhase::ProjectionUnavailable,
            SessionPhaseKind::MoveProjectionUnavailable => {
                NativeDisplayPhase::MoveProjectionUnavailable
            }
            SessionPhaseKind::PanelProjectionUnavailable => {
                NativeDisplayPhase::PanelProjectionUnavailable
            }
            SessionPhaseKind::RenameProjectionUnavailable => {
                NativeDisplayPhase::RenameProjectionUnavailable
            }
            SessionPhaseKind::Switching => NativeDisplayPhase::Switching,
            SessionPhaseKind::SelectedWithoutAttachment => {
                NativeDisplayPhase::SelectedWithoutAttachment
            }
            SessionPhaseKind::RetryableAttachUnavailable => {
                NativeDisplayPhase::RetryableAttachUnavailable
            }
            SessionPhaseKind::Retired => NativeDisplayPhase::Retired,
        }
    }

    pub(crate) fn tab_count(&self) -> usize {
        self.state.as_ref().map_or(0, StateSessionView::tab_count)
    }

    pub(crate) fn tab(&self, index: usize) -> Option<NativeTabView<'a>> {
        self.state
            .as_ref()?
            .tab(index)
            .map(|inner| NativeTabView { inner })
    }

    pub(crate) fn active_position(&self) -> Option<usize> {
        self.state.as_ref()?.active_position()
    }

    pub(crate) fn controller(&self) -> Option<NativeDisplayController> {
        self.state
            .as_ref()?
            .controller_mode()
            .map(|mode| match mode {
                ControllerMode::Controlling(_) => NativeDisplayController::Controlling,
                ControllerMode::ReadOnly => NativeDisplayController::ReadOnly,
            })
    }

    pub(crate) fn selected_provenance(&self) -> Option<NativeDisplayProvenance> {
        self.state
            .as_ref()?
            .selected_provenance()
            .map(|provenance| match provenance {
                CreateTerminalProvenance::Started => NativeDisplayProvenance::Started,
                CreateTerminalProvenance::Pending => NativeDisplayProvenance::Pending,
                CreateTerminalProvenance::Unavailable(_) => NativeDisplayProvenance::Unavailable,
                CreateTerminalProvenance::Failed(_) => NativeDisplayProvenance::Failed,
                CreateTerminalProvenance::OutcomeUnknown => NativeDisplayProvenance::OutcomeUnknown,
                CreateTerminalProvenance::BindingOnly => NativeDisplayProvenance::BindingOnly,
            })
    }

    pub(crate) fn notice(&self) -> Option<NativeSessionNotice> {
        self.state.as_ref()?.notice().map(|notice| match notice {
            SessionNotice::LocalBusy => NativeSessionNotice::LocalBusy,
            SessionNotice::NoTabAtPosition(position) => {
                NativeSessionNotice::NoTabAtPosition(position)
            }
            SessionNotice::CreateNotCreated => NativeSessionNotice::CreateNotCreated,
            SessionNotice::MoveNotMoved => NativeSessionNotice::MoveNotMoved,
            SessionNotice::PanelNotApplied => NativeSessionNotice::PanelNotApplied,
            SessionNotice::RenameNotRenamed => NativeSessionNotice::RenameNotRenamed,
            SessionNotice::CloseRequestNotApplied => NativeSessionNotice::CloseRequestNotApplied,
        })
    }

    pub(crate) fn refresh_status(&self) -> Option<NativeRefreshStatus> {
        self.state
            .as_ref()?
            .refresh_status()
            .map(|status| match status {
                NavigatorRefreshStatus::Refreshing => NativeRefreshStatus::Refreshing,
                NavigatorRefreshStatus::Success(revision) => {
                    NativeRefreshStatus::Success(revision.value())
                }
                NavigatorRefreshStatus::Failure(revision) => {
                    NativeRefreshStatus::Failure(revision.value())
                }
            })
    }

    pub(crate) const fn projection_status(&self) -> NativeProjectionStatus {
        self.projection_status
    }

    pub(crate) const fn projection(&self) -> Option<&'a TerminalProjection> {
        self.projection
    }

    pub(crate) const fn retirement(&self) -> Option<NativeDisplayRetirement> {
        self.retirement
    }
}

struct KeyedProjection {
    key: AttachmentKey,
    value: TerminalProjection,
}

struct PendingDraw {
    poll_key: super::mailbox::PollKey,
    settlement: CausalSettlement,
    frame: Option<FrameIdentity>,
    ordinary: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActionBuildError {
    Busy,
    NoOp,
    OutOfRange(u8),
    Protocol,
}

pub(super) struct NativeUi {
    mailbox: Option<Arc<CommandMailbox>>,
    clock: LocalOperationClock,
    state: Option<NativeSessionState>,
    projection: Option<KeyedProjection>,
    projection_status: NativeProjectionStatus,
    pending_draw: Option<PendingDraw>,
    ordinary_poll_serial: Option<u64>,
    retirement_failure: Option<CarrierFailure>,
    retirement_finalized: bool,
    retirement: Option<NativeDisplayRetirement>,
    #[cfg(target_os = "linux")]
    retired_mailbox_seed: Option<super::mailbox::RetiredMailboxSeed>,
    #[cfg(target_os = "linux")]
    retirement_poll_serial: Option<u64>,
    #[cfg(target_os = "linux")]
    retired_peer: Option<super::state::retired_membership::RetiredMembershipOrigin>,
    #[cfg(target_os = "linux")]
    retired_peer_attempt: Option<retired_membership::RetiredMembershipAttempt>,
}

#[cfg(all(test, unix))]
mod context_selection_tests;
#[cfg(test)]
mod create_membership_tests;
#[cfg(test)]
mod location_labels_tests;
#[cfg(all(test, unix))]
mod membership_tests;
#[cfg(all(test, unix))]
mod move_membership_tests;
#[cfg(all(test, unix))]
mod panel_input_tests;
#[cfg(all(test, unix))]
mod publication_hint_tests;
#[cfg(test)]
mod rename_membership_tests;
#[cfg(all(test, target_os = "linux"))]
mod retired_membership_tests;
#[cfg(test)]
mod workspace_setup_tests;

impl NativeUi {
    pub(super) fn new(mailbox: Arc<CommandMailbox>, clock: LocalOperationClock) -> Self {
        Self {
            mailbox: Some(mailbox),
            clock,
            state: None,
            projection: None,
            projection_status: NativeProjectionStatus::Empty,
            pending_draw: None,
            ordinary_poll_serial: None,
            retirement_failure: None,
            retirement_finalized: false,
            retirement: None,
            #[cfg(target_os = "linux")]
            retired_mailbox_seed: None,
            #[cfg(target_os = "linux")]
            retirement_poll_serial: None,
            #[cfg(target_os = "linux")]
            retired_peer: None,
            #[cfg(target_os = "linux")]
            retired_peer_attempt: None,
        }
    }

    pub(super) fn view(&self) -> NativeSessionView<'_> {
        let state = self.state.as_ref();
        let projection = self
            .projection
            .as_ref()
            .filter(|projection| {
                state.is_some_and(|state| state.projection_key() == Some(&projection.key))
            })
            .map(|projection| &projection.value);
        NativeSessionView {
            state: state.map(NativeSessionState::view),
            projection,
            projection_status: self.projection_status,
            retirement: self.retirement,
        }
    }

    #[cfg(test)]
    pub(super) fn projection(&self) -> Option<&TerminalProjection> {
        self.view().projection()
    }

    pub(super) fn status(&self) -> super::TerminalStatus {
        use super::TerminalStatus;
        let Some(state) = self.state.as_ref() else {
            return match self.retirement {
                Some(NativeDisplayRetirement::OutcomeUnknown) => TerminalStatus::OutcomeUnknown,
                Some(NativeDisplayRetirement::Unavailable) => TerminalStatus::Unavailable,
                None => TerminalStatus::Connecting,
            };
        };
        if self.projection_status == NativeProjectionStatus::Blocked {
            return TerminalStatus::Blocked;
        }
        match (state.phase_kind(), state.controller_mode()) {
            (
                SessionPhaseKind::Ready
                | SessionPhaseKind::MoveProjectionUnavailable
                | SessionPhaseKind::RenameProjectionUnavailable,
                Some(ControllerMode::Controlling(_)),
            ) => TerminalStatus::Controlling,
            (
                SessionPhaseKind::Ready
                | SessionPhaseKind::MoveProjectionUnavailable
                | SessionPhaseKind::RenameProjectionUnavailable,
                Some(ControllerMode::ReadOnly),
            ) => TerminalStatus::ReadOnly,
            (SessionPhaseKind::Retired, _) => match self.retirement {
                Some(NativeDisplayRetirement::OutcomeUnknown) => TerminalStatus::OutcomeUnknown,
                Some(NativeDisplayRetirement::Unavailable) | None => TerminalStatus::Unavailable,
            },
            (
                SessionPhaseKind::SelectedWithoutAttachment
                | SessionPhaseKind::RetryableAttachUnavailable
                | SessionPhaseKind::ProjectionUnavailable,
                _,
            ) => TerminalStatus::Unavailable,
            _ => TerminalStatus::Connecting,
        }
    }

    pub(super) fn startup_draw_ready(&self) -> bool {
        self.pending_draw.is_some()
    }

    pub(super) const fn retirement_requested(&self) -> bool {
        self.retirement_failure.is_some() && !self.retirement_finalized
    }

    pub(super) fn release_mailbox(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(mailbox) = &self.mailbox {
            self.retired_mailbox_seed = Some(mailbox.retired_seed());
        }
        self.mailbox.take();
    }

    #[cfg(test)]
    pub(super) fn committed_target_retained_for_test(&self) -> bool {
        self.state
            .as_ref()
            .and_then(NativeSessionState::committed_target)
            .is_some()
    }

    #[cfg(test)]
    pub(super) fn move_retirement_for_test(&self) -> Option<(&PendingMoveCommit, CarrierFailure)> {
        let state = self.state.as_ref()?;
        Some((state.retained_move_for_test()?, state.retired_failure()?))
    }

    #[cfg(test)]
    pub(super) fn latest_geometry_for_test(&self) -> Option<Geometry> {
        self.state.as_ref().map(NativeSessionState::latest_geometry)
    }

    pub(super) fn close(&mut self, failure: CarrierFailure) {
        self.retire(failure);
    }

    pub(super) fn finalize_retirement(
        &mut self,
        pending_create: Option<PendingCreateCommit>,
        pending_move: Option<PendingMoveCommit>,
        pending_panel: Option<PendingPanelCommit>,
        close_return: super::state::CloseWorkerReturn,
    ) -> bool {
        if self.retirement_finalized {
            return pending_create.is_none()
                && pending_move.is_none()
                && pending_panel.is_none()
                && matches!(
                    close_return,
                    super::state::CloseWorkerReturn::Returned(None)
                );
        }
        let failure = self
            .retirement_failure
            .or_else(|| {
                self.mailbox
                    .as_ref()
                    .and_then(|mailbox| mailbox.is_closed())
            })
            .unwrap_or(CarrierFailure::WorkerExited);
        let accepted = match self.state.as_mut() {
            Some(state) => {
                let move_accepted =
                    pending_move.is_none_or(|pending| state.retain_pending_move(pending));
                let panel_accepted =
                    pending_panel.is_none_or(|pending| state.retain_pending_panel(pending));
                let close_accepted = state.finalize_close_worker(close_return).is_ok();
                state.release_joined_close_references();
                state.finalize_retirement(failure, pending_create)
                    && move_accepted
                    && panel_accepted
                    && close_accepted
            }
            None => pending_create.is_none() && pending_move.is_none() && pending_panel.is_none(),
        };
        let failure = self
            .state
            .as_ref()
            .and_then(NativeSessionState::retired_failure)
            .unwrap_or(failure);
        self.retirement_failure = Some(failure);
        self.clear_projection();
        self.retirement_finalized = true;
        self.retirement = Some(if failure == CarrierFailure::OutcomeUnknown {
            NativeDisplayRetirement::OutcomeUnknown
        } else {
            NativeDisplayRetirement::Unavailable
        });
        accepted
    }

    pub(super) fn tick(&mut self) -> bool {
        if self.retirement_failure.is_some() {
            return false;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return false;
        };
        let acknowledged = if let Some(ack) = mailbox.take_close_acknowledgement()
            && let Some(state) = &mut self.state
        {
            state.acknowledge_close_lease(ack);
            true
        } else {
            false
        };
        if let Some(failure) = mailbox.is_closed() {
            self.retire(failure);
            return true;
        }
        let taken = match mailbox.take_event_before(&self.clock) {
            Ok(taken) => taken,
            Err(EventError::ForwardExpired) => return acknowledged || self.pending_draw.is_some(),
            Err(EventError::Closed(closed)) => {
                self.retire(closed.failure());
                return true;
            }
        };
        let changed = match taken {
            Some(taken) => self.apply_event(&mailbox, taken),
            None => false,
        };
        if self.retirement_failure.is_none() {
            self.reconcile_ordinary_poll(&mailbox);
            self.drain_publication_refresh(&mailbox);
            self.schedule_close_reconciliation(&mailbox);
            self.schedule_close_read(&mailbox);
            self.schedule_poll(&mailbox);
        }
        acknowledged || changed || self.pending_draw.is_some()
    }

    pub(super) fn after_draw(&mut self) {
        let Some(pending) = self.pending_draw.take() else {
            return;
        };
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return;
        };
        if let Some(frame) = pending.frame {
            let current = self.state.as_ref().is_some_and(|state| {
                state.desired() == Some(pending.poll_key.attachment())
                    && state.projection_key() == Some(pending.poll_key.attachment())
            });
            if current
                && self.state.as_mut().is_none_or(|state| {
                    !state.stage_drawn_frame(pending.poll_key.attachment(), frame)
                })
            {
                self.retire(CarrierFailure::Protocol);
                return;
            }
        }
        if pending.ordinary && pending.frame.is_none() {
            self.ordinary_poll_serial = None;
        }
        if let Err(closed) = mailbox.deposit_causal(&pending.poll_key, pending.settlement) {
            self.retire(closed.failure());
        }
    }

    pub(super) fn observe_geometry(&mut self, geometry: Geometry) {
        if self
            .state
            .as_ref()
            .is_some_and(|state| state.latest_geometry() != geometry)
        {
            self.invalidate_close_display();
            self.invalidate_rename_display();
            self.invalidate_setup_display();
        }
        if self.retirement_failure.is_some() {
            return;
        }
        if let Some(state) = self.state.as_mut() {
            state.observe_geometry(geometry);
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return;
        };
        if let Err(closed) = mailbox.observe_geometry(geometry) {
            self.retire(closed.failure());
            return;
        }
        let desired = self.state.as_ref().and_then(|state| {
            let ready = state.ready()?;
            matches!(ready.mode(), ControllerMode::Controlling(_))
                .then(|| ready.selected().key().clone())
        });
        let Some(desired) = desired else {
            return;
        };
        if self
            .state
            .as_ref()
            .is_some_and(|state| state.close_suppresses_attachment(&desired))
        {
            return;
        }
        if self
            .state
            .as_ref()
            .and_then(|state| state.last_admitted_geometry(&desired))
            == Some(geometry)
        {
            return;
        }
        match mailbox.try_enqueue(QueuedCommand::Resize(Tagged::new(
            desired.clone(),
            geometry,
        ))) {
            Ok(_serial) => {
                if self
                    .state
                    .as_mut()
                    .is_none_or(|state| !state.record_admitted_geometry(&desired, geometry))
                {
                    self.retire(CarrierFailure::Protocol);
                }
            }
            Err(AdmissionError::Full) => {}
            Err(AdmissionError::Closed(closed)) => self.retire(closed.failure()),
            Err(
                AdmissionError::BootstrapPending
                | AdmissionError::Busy
                | AdmissionError::StaleAttachment
                | AdmissionError::AttachmentGenerationExhausted
                | AdmissionError::RefreshGenerationExhausted
                | AdmissionError::Build(_),
            ) => self.retire(CarrierFailure::Protocol),
        }
    }

    pub(super) fn forward(&mut self, input: &[u8]) -> bool {
        if self.retirement_failure.is_some() {
            return false;
        }
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let Some(ready) = state.ready() else {
            return false;
        };
        if !matches!(ready.mode(), ControllerMode::Controlling(_)) {
            return false;
        }
        let desired = ready.selected().key().clone();
        if state.close_suppresses_attachment(&desired) {
            return false;
        }
        let Ok(input) = ControlInput::new(input.to_vec()) else {
            return false;
        };
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return false;
        };
        match mailbox.try_enqueue(QueuedCommand::Input(Tagged::new(desired, input))) {
            Ok(_serial) => true,
            Err(AdmissionError::Full) => false,
            Err(AdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                false
            }
            Err(
                AdmissionError::BootstrapPending
                | AdmissionError::Busy
                | AdmissionError::StaleAttachment
                | AdmissionError::AttachmentGenerationExhausted
                | AdmissionError::RefreshGenerationExhausted
                | AdmissionError::Build(_),
            ) => {
                self.retire(CarrierFailure::Protocol);
                false
            }
        }
    }

    pub(super) fn apply_client_action(&mut self, action: ClientAction) -> SessionActionDisposition {
        if self.retirement_failure.is_some() {
            return SessionActionDisposition::KeepNavigator;
        }
        if let Some(state) = self.state.as_mut() {
            state.clear_settled_setup_status();
        }
        match action {
            ClientAction::RequestWorkspaceCreate
            | ClientAction::RequestWorkspaceDirectory
            | ClientAction::RequestWorkspaceRename
            | ClientAction::RequestProjectRename => {
                use super::state::workspace_setup::NativeSetupIntent;
                let intent = match action {
                    ClientAction::RequestWorkspaceCreate => NativeSetupIntent::CreateWorkspace,
                    ClientAction::RequestWorkspaceDirectory => {
                        NativeSetupIntent::ConfigureDirectory
                    }
                    ClientAction::RequestWorkspaceRename => {
                        NativeSetupIntent::RenameLocation(NativeLocationLabelTarget::Workspace)
                    }
                    _ => NativeSetupIntent::RenameLocation(NativeLocationLabelTarget::Project),
                };
                self.request_setup_draft(intent);
                SessionActionDisposition::KeepNavigator
            }
            ClientAction::RequestRenameTab => {
                if self.request_rename_draft().is_some() {
                    SessionActionDisposition::DismissNavigator
                } else {
                    SessionActionDisposition::KeepNavigator
                }
            }
            ClientAction::RequestCloseTab => self.request_close_confirmation(),
            ClientAction::ConfirmClose => self.confirm_close(),
            ClientAction::CancelClose => self.cancel_close(),
            ClientAction::CreateTab => self.create_tab(),
            ClientAction::MovePreviousTab => self.move_tab(WorkbenchTabMoveDirection::Previous),
            ClientAction::MoveNextTab => self.move_tab(WorkbenchTabMoveDirection::Next),
            ClientAction::SelectPreviousContext => self.select_context(false),
            ClientAction::SelectNextContext => self.select_context(true),
            ClientAction::SelectNextTab
            | ClientAction::SelectPreviousTab
            | ClientAction::SelectTab(_) => self.select_tab(action),
            ClientAction::MovePanelFocus(direction) => self.move_panel_focus(direction),
            ClientAction::SplitPanel(direction) => {
                self.panel_operation(PanelRequestKind::Split { direction })
            }
            ClientAction::ClosePanel => self.panel_operation(PanelRequestKind::Close),
            ClientAction::MovePanel(direction) => {
                self.panel_operation(PanelRequestKind::Move { direction })
            }
            ClientAction::OpenWorkbenchNavigator => self.open_navigator(),
            ClientAction::ToggleBusinessSidebar
            | ClientAction::OpenShortcutHelp
            | ClientAction::CloseWorkbenchNavigator
            | ClientAction::ForwardLiteralPrefix
            | ClientAction::DismissPrefix
            | ClientAction::UnknownPrefix
            | ClientAction::Exit => {
                self.clear_notice();
                SessionActionDisposition::KeepNavigator
            }
        }
    }

    fn clear_notice(&mut self) {
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return;
        };
        mailbox.apply_local_for(state, NativeSessionState::clear_notice);
    }

    fn create_tab(&mut self) -> SessionActionDisposition {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(CarrierFailure::Deadline);
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_create_for(
            state,
            |state, generation, request_id| build_create(state, generation, request_id, deadlines),
            finish_action,
        );
        match result {
            Ok(_operation) => SessionActionDisposition::DismissNavigator,
            Err(error) => {
                self.retire_for_admission(&error);
                SessionActionDisposition::KeepNavigator
            }
        }
    }

    fn select_tab(&mut self, action: ClientAction) -> SessionActionDisposition {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(CarrierFailure::Deadline);
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_switch_for(
            state,
            |state, generation| build_switch(state, action, generation, deadlines),
            |state, result, facts| {
                if result.is_ok() {
                    state.finish_switch_membership_admission(
                        facts.discarded(),
                        facts.inherited(),
                        facts.authorized_cleanup(),
                    );
                }
                finish_action(state, result);
            },
        );
        match result {
            Ok(_generation) => {
                // State hides the old projection while retaining the exact renderer for a proven restore.
                self.pending_draw = None;
                self.ordinary_poll_serial = None;
                SessionActionDisposition::DismissNavigator
            }
            Err(error) => {
                self.retire_for_admission(&error);
                SessionActionDisposition::KeepNavigator
            }
        }
    }

    fn select_context(&mut self, next: bool) -> SessionActionDisposition {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(CarrierFailure::Deadline);
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_context_selection_for(
            state,
            deadlines,
            &self.clock,
            |state, generation| {
                state
                    .start_context_selection(generation, next, deadlines)
                    .then_some(())
                    .ok_or(ActionBuildError::Busy)
            },
            finish_action,
        );
        if let Err(error) = result {
            self.retire_for_admission(&error);
        }
        SessionActionDisposition::KeepNavigator
    }

    fn move_tab(&mut self, direction: WorkbenchTabMoveDirection) -> SessionActionDisposition {
        // Capture the entire transport budget before attempting the admission mutex.
        let Some(deadline) = self.clock.now().checked_add(super::state::OPERATION_TOTAL) else {
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_move_for(
            state,
            deadline,
            &self.clock,
            |state, request_id| {
                state
                    .plan_move(request_id, direction, deadline)
                    .ok_or(ActionBuildError::Busy)
            },
            |state, operation| {
                state
                    .begin_move(operation.clone())
                    .then_some(())
                    .ok_or(ActionBuildError::Busy)
            },
        );
        match result {
            Ok(_) => SessionActionDisposition::DismissNavigator,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                SessionActionDisposition::KeepNavigator
            }
            // A deadline refusal can return while another thread still holds the mailbox.
            // Preserve local state and do not reacquire that lock to update a notice.
            Err(_) => SessionActionDisposition::KeepNavigator,
        }
    }

    fn panel_operation(&mut self, kind: PanelRequestKind) -> SessionActionDisposition {
        // Capture the entire transport budget before attempting the admission mutex.
        let Some(deadline) = self.clock.now().checked_add(super::state::OPERATION_TOTAL) else {
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_panel_for(
            state,
            deadline,
            &self.clock,
            |state, request_id| {
                state
                    .plan_panel(request_id, kind, deadline)
                    .ok_or(ActionBuildError::Busy)
            },
            |state, operation| {
                state
                    .begin_panel(operation.clone())
                    .then_some(())
                    .ok_or(ActionBuildError::Busy)
            },
        );
        match result {
            Ok(_) => SessionActionDisposition::DismissNavigator,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                SessionActionDisposition::KeepNavigator
            }
            Err(_) => SessionActionDisposition::KeepNavigator,
        }
    }

    fn move_panel_focus(&mut self, direction: PanelDirection) -> SessionActionDisposition {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_switch_for(
            state,
            |state, generation| build_panel_switch(state, generation, direction, deadlines),
            |state, result, facts| {
                if result.is_ok() {
                    state.finish_switch_membership_admission(
                        facts.discarded(),
                        facts.inherited(),
                        facts.authorized_cleanup(),
                    );
                }
                finish_action(state, result);
            },
        );
        match result {
            Ok(_) => SessionActionDisposition::KeepNavigator,
            Err(AdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                SessionActionDisposition::KeepNavigator
            }
            Err(_) => SessionActionDisposition::KeepNavigator,
        }
    }

    fn open_navigator(&mut self) -> SessionActionDisposition {
        if self
            .state
            .as_ref()
            .is_some_and(NativeSessionState::setup_recovery)
        {
            return self.refresh_workspace_setup();
        }
        let mut reconcile = false;
        if let Some(state) = &mut self.state {
            state.request_close_manual_pass();
            if state.close_status_only() {
                reconcile = state.request_close_membership_recovery().is_ok();
                if !reconcile {
                    return SessionActionDisposition::KeepNavigator;
                }
            } else {
                state.request_live_close_reconciliation();
                reconcile = state.close_reconciliation_requested_origin().is_some();
            }
        }
        if reconcile {
            if let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) {
                self.schedule_close_reconciliation(&mailbox);
            }
            return SessionActionDisposition::KeepNavigator;
        }
        let pending = {
            let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
                return SessionActionDisposition::KeepNavigator;
            };
            mailbox.apply_local_for(state, |state| {
                if state.pending_refresh_generation().is_some() {
                    state.clear_notice();
                    true
                } else {
                    false
                }
            })
        };
        if pending {
            return SessionActionDisposition::KeepNavigator;
        }
        let Some(deadline) = self.clock.now().checked_add(super::state::OPERATION_TOTAL) else {
            self.retire(CarrierFailure::Deadline);
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_refresh_for(
            state,
            deadline,
            |state, generation| {
                state
                    .start_pending_refresh(generation)
                    .then_some(())
                    .ok_or(ActionBuildError::Busy)
            },
            finish_action,
        );
        if let Err(error) = result {
            self.retire_for_admission(&error);
        }
        SessionActionDisposition::KeepNavigator
    }

    fn retire_for_admission(&mut self, error: &AdmissionError<ActionBuildError>) {
        match error {
            AdmissionError::Closed(closed) => self.retire(closed.failure()),
            AdmissionError::AttachmentGenerationExhausted
            | AdmissionError::RefreshGenerationExhausted => {
                self.retire(CarrierFailure::IdentityExhausted);
            }
            AdmissionError::Build(ActionBuildError::Protocol) => {
                self.retire(CarrierFailure::Protocol);
            }
            AdmissionError::BootstrapPending
            | AdmissionError::Busy
            | AdmissionError::Full
            | AdmissionError::StaleAttachment
            | AdmissionError::Build(
                ActionBuildError::Busy | ActionBuildError::NoOp | ActionBuildError::OutOfRange(_),
            ) => {}
        }
    }

    fn apply_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        taken: super::mailbox::TakenEvent,
    ) -> bool {
        let (wait_key, event) = taken.into_parts();
        if let WorkerEvent::PollResult {
            poll_key, record, ..
        } = event
        {
            if wait_key.is_some() {
                self.retire(CarrierFailure::Protocol);
                return true;
            }
            return self.apply_poll_result(mailbox, poll_key, record);
        }
        let Some(wait_key) = wait_key else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        if let WorkerEvent::Bootstrap { cache } = event {
            if self.state.is_some() {
                self.retire(CarrierFailure::Protocol);
                return true;
            }
            let mut installed = None;
            let result = mailbox.deposit_bootstrap_reply_before(
                &wait_key,
                &self.clock,
                |generation, geometry| {
                    let (state, target) = NativeSessionState::starting(cache, generation, geometry)
                        .ok_or(CarrierFailure::AuthorityDivergence)?;
                    installed = Some(state);
                    Ok(target)
                },
            );
            match result {
                Ok(MutationResult::Applied) => {
                    self.state = installed;
                    return self.validate_state();
                }
                Ok(MutationResult::Stale) => return false,
                Err(error) => return self.handle_event_error(error),
            }
        }
        let event = match event {
            WorkerEvent::CreateSnapshotResult { operation, outcome } => {
                return self.apply_create_snapshot_event(mailbox, wait_key, operation, outcome);
            }
            WorkerEvent::CloseReconciliationSnapshot { marker, outcome } => {
                return self.apply_close_reconciliation_event(mailbox, wait_key, marker, outcome);
            }
            WorkerEvent::MembershipGone { cleanup, proof } => {
                if self
                    .state
                    .as_ref()
                    .is_none_or(|state| !state.validate_membership_gone(&cleanup, &proof))
                {
                    self.retire(CarrierFailure::Protocol);
                    return true;
                }
                let old = proof.attachment_key().clone();
                return self.apply_membership_detached(mailbox, wait_key, cleanup, old);
            }
            WorkerEvent::SwitchForwardExpired { permit } => {
                return self.apply_switch_forward_expired(mailbox, wait_key, permit.into_target());
            }
            WorkerEvent::SwitchSnapshotResult { marker, outcome } => {
                return self.apply_switch_snapshot(mailbox, wait_key, marker, outcome);
            }
            WorkerEvent::MembershipCleanupSkipped { cleanup } => {
                return self.apply_switch_cleanup_skipped(mailbox, wait_key, cleanup);
            }
            WorkerEvent::MembershipDetached { cleanup, detached } => {
                return self.apply_membership_detached(mailbox, wait_key, cleanup, detached);
            }
            WorkerEvent::CloseResult { operation, outcome } => {
                return self.apply_close_event(mailbox, wait_key, operation, outcome);
            }
            WorkerEvent::MoveResult { operation, outcome } => {
                return self.apply_move_event(mailbox, wait_key, operation, outcome);
            }
            WorkerEvent::PanelResult { operation, outcome } => {
                return self.apply_panel_event(mailbox, wait_key, operation, outcome);
            }
            WorkerEvent::WorkspaceSetupResult { operation, outcome } => {
                return self.apply_workspace_setup_event(mailbox, wait_key, operation, outcome);
            }
            WorkerEvent::RenameResult { operation, outcome } => {
                return self.apply_rename_event(mailbox, wait_key, operation, outcome);
            }
            WorkerEvent::RefreshSnapshotResult {
                generation,
                outcome,
            } => return self.apply_refresh_event(mailbox, wait_key, generation, outcome),
            event => event,
        };
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut clear_projection = false;
        let mut install_projection = None;
        let result = mailbox.deposit_ui_reply_before(&wait_key, &self.clock, |transaction| {
            let reply = match event {
                WorkerEvent::WorkspaceSetupResult { .. }
                | WorkerEvent::RenameResult { .. }
                | WorkerEvent::MoveResult { .. }
                | WorkerEvent::PanelResult { .. }
                | WorkerEvent::CloseReconciliationSnapshot { .. }
                | WorkerEvent::MembershipGone { .. }
                | WorkerEvent::MembershipDetached { .. }
                | WorkerEvent::CloseResult { .. }
                | WorkerEvent::SwitchSnapshotResult { .. }
                | WorkerEvent::MembershipCleanupSkipped { .. } => {
                    return Err(CarrierFailure::Protocol);
                }
                WorkerEvent::SwitchForwardExpired { .. } => return Err(CarrierFailure::Protocol),
                WorkerEvent::SetupConfigured {
                    key,
                    mode,
                    applied_geometry,
                } => UiReply::SetupConfigurationSettled(
                    state
                        .settle_setup_configuration_event(&key, mode, applied_geometry)
                        .ok_or(CarrierFailure::Protocol)?,
                ),
                WorkerEvent::SwitchEntryExpired {
                    key,
                    target_index,
                    target_provenance,
                } => UiReply::SwitchEntryExpiredApplied(
                    state
                        .settle_switch_entry_expired(
                            &key,
                            target_index,
                            &target_provenance,
                            transaction.observed_at(),
                        )
                        .ok_or(CarrierFailure::Protocol)?,
                ),
                WorkerEvent::SetupWithoutAttachment {
                    key,
                    target_index,
                    target_provenance,
                } => {
                    let continuation = state
                        .settle_unattached_setup(&key, target_index, &target_provenance)
                        .ok_or(CarrierFailure::Protocol)?;
                    clear_projection = true;
                    UiReply::SetupWithoutAttachmentApplied(continuation)
                }
                WorkerEvent::CreatePrecommit {
                    operation,
                    outcome,
                    applied_geometry,
                } => {
                    let notice = create_precommit_notice(&outcome);
                    if transaction.latest_geometry() != state.latest_geometry()
                        || !state.restore_precommit(&operation, applied_geometry)
                    {
                        return Err(CarrierFailure::Protocol);
                    }
                    state.set_notice(notice);
                    if matches!(
                        state.controller_mode(),
                        Some(ControllerMode::Controlling(_))
                    ) && Some(transaction.latest_geometry()) != applied_geometry
                    {
                        let desired = state.desired().cloned().ok_or(CarrierFailure::Protocol)?;
                        let geometry = transaction.latest_geometry();
                        transaction.enqueue_catch_up_resize(desired.clone(), geometry)?;
                        if !state.record_admitted_geometry(&desired, geometry) {
                            return Err(CarrierFailure::Protocol);
                        }
                    }
                    UiReply::PrecommitRestored
                }
                WorkerEvent::CreateCommittedKnown { operation, commit } => state
                    .retain_create_commit(&operation, commit)
                    .then_some(UiReply::CommitRetained)
                    .ok_or(CarrierFailure::Protocol)?,
                WorkerEvent::CreateSnapshotResult { .. } => return Err(CarrierFailure::Protocol),
                WorkerEvent::Attached { key, projection } => {
                    let disposition = state
                        .settle_attached_event(&key)
                        .ok_or(CarrierFailure::Protocol)?;
                    if disposition == SetupEventDisposition::Applied {
                        install_projection = Some(KeyedProjection {
                            key,
                            value: projection,
                        });
                    }
                    UiReply::AttachedSettled(disposition)
                }
                WorkerEvent::FeedbackAccepted { key, frame } => {
                    let disposition = state
                        .settle_feedback_event(&key, frame)
                        .ok_or(CarrierFailure::Protocol)?;
                    if disposition == SetupEventDisposition::Applied
                        && self.ordinary_poll_serial.map(FrameIdentity::new) == Some(frame)
                    {
                        self.ordinary_poll_serial = None;
                    }
                    UiReply::FeedbackSettled(disposition)
                }
                WorkerEvent::ReadyCandidate {
                    key,
                    mode,
                    last_accepted_geometry,
                    frame,
                } => UiReply::Ready(state.settle_ready_candidate(
                    &key,
                    frame,
                    mode,
                    last_accepted_geometry,
                )?),
                WorkerEvent::CleanupSettled {
                    key,
                    target_index,
                    target_provenance,
                } => {
                    let continuation = state
                        .settle_cleanup(&key, target_index, &target_provenance)
                        .ok_or(CarrierFailure::Protocol)?;
                    clear_projection = true;
                    UiReply::CleanupApplied(continuation)
                }
                WorkerEvent::Bootstrap { .. }
                | WorkerEvent::RefreshSnapshotResult { .. }
                | WorkerEvent::PollResult { .. } => {
                    return Err(CarrierFailure::Protocol);
                }
            };
            Ok(reply)
        });
        match result {
            Ok(MutationResult::Applied) => {
                if clear_projection {
                    self.clear_projection();
                }
                if let Some(projection) = install_projection {
                    self.projection = Some(projection);
                    self.projection_status = NativeProjectionStatus::Empty;
                }
                self.validate_state()
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }

    fn apply_refresh_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait_key: super::mailbox::UiWaitKey,
        generation: u64,
        outcome: orchbee_workbench_protocol::WorkbenchSnapshotOutcome,
    ) -> bool {
        #[cfg(target_os = "linux")]
        if self.retired_peer_attempt.is_some() {
            return self.apply_retired_membership_result(mailbox, wait_key, generation, outcome);
        }
        let Some(state) = self.state.as_ref() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        // Projection and append-only validation are bounded but intentionally run outside the
        // mailbox mutex. Only the prepared value is committed in the exact reply transaction.
        let prepared = state.prepare_refresh(generation, outcome);
        let recovery_deadlines = state
            .setup_deadlines()
            .or_else(|| state.context_selection_deadlines())
            .or_else(|| {
                prepared
                    .needs_recovery_deadlines()
                    .then(|| OperationDeadlines::from_sample(self.clock.now()))
                    .flatten()
            });
        if let super::state::PreparedRefresh::CreateMembership {
            generation,
            operation,
            prepared,
        } = prepared
        {
            return self.apply_create_recovery_event(
                mailbox,
                wait_key,
                generation,
                operation,
                prepared,
                recovery_deadlines,
            );
        }
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut clear_projection = false;
        let mut retire_after_reply = None;
        let result = mailbox.deposit_ui_reply_before(&wait_key, &self.clock, |transaction| {
            let decision = if let super::state::PreparedRefresh::Membership {
                generation,
                prepared,
            } = prepared
            {
                if !prepared.preserves_selection() && recovery_deadlines.is_none() {
                    return Err(CarrierFailure::Deadline);
                }
                if recovery_deadlines.is_some_and(|deadlines| {
                    transaction.observed_at() >= deadlines.forward_cutoff()
                }) {
                    return Err(CarrierFailure::Deadline);
                }
                if state.attached_key().is_some() && !prepared.preserves_selection() {
                    let cleanup = state.stage_refresh_membership_cleanup(
                        generation,
                        prepared,
                        recovery_deadlines.ok_or(CarrierFailure::Deadline)?,
                    )?;
                    return Ok(UiReply::Refresh(RefreshSettlement::new(
                        RefreshDecision::Cleanup(cleanup),
                        state.desired().cloned(),
                    )));
                }
                let install = if prepared.preserves_selection() {
                    super::mailbox::DesiredInstall::Preserve
                } else if let Some(tab) = prepared.selected_tab_id() {
                    super::mailbox::DesiredInstall::Fresh(tab.clone())
                } else {
                    super::mailbox::DesiredInstall::Empty
                };
                let desired_plan = transaction.prepare_desired_install(install)?;
                let state_plan = state.prepare_refresh_membership_install(
                    generation,
                    prepared,
                    desired_plan.desired().cloned(),
                    recovery_deadlines,
                )?;
                let decision = state_plan.commit();
                desired_plan.commit();
                match decision {
                    super::state::MembershipInstallDecision::Preserved => RefreshDecision::Continue,
                    super::state::MembershipInstallDecision::Attach(target) => {
                        RefreshDecision::AttachRecovered(target)
                    }
                    super::state::MembershipInstallDecision::SelectedWithoutAttachment
                    | super::state::MembershipInstallDecision::Empty => {
                        clear_projection = true;
                        RefreshDecision::SettledWithoutAttachment
                    }
                }
            } else {
                state.apply_prepared_refresh(prepared)
            };
            if matches!(decision, RefreshDecision::Retire) {
                clear_projection = true;
                retire_after_reply =
                    Some(state.retired_failure().unwrap_or(CarrierFailure::Protocol));
            }
            Ok(UiReply::Refresh(RefreshSettlement::new(
                decision,
                state.desired().cloned(),
            )))
        });
        match result {
            Ok(MutationResult::Applied) => {
                if clear_projection {
                    self.clear_projection();
                }
                if let Some(failure) = retire_after_reply {
                    self.retire(failure);
                    return true;
                }
                self.validate_state()
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }

    /// Applies a Worker poll result. A committed publication hint is owned by the mailbox
    /// ingress latch, so consuming the event here must not produce refresh debt again.
    fn apply_poll_result(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        poll_key: super::mailbox::PollKey,
        record: Option<Vec<u8>>,
    ) -> bool {
        let ordinary = self.ordinary_poll_serial == Some(poll_key.serial());
        let Some(record) = record else {
            if ordinary {
                self.ordinary_poll_serial = None;
            }
            return match mailbox.complete_empty_poll(&poll_key) {
                Ok(MutationResult::Applied | MutationResult::Stale) => false,
                Err(closed) => {
                    self.retire(closed.failure());
                    true
                }
            };
        };
        let expected = match self.state.as_ref() {
            Some(state) => state.expected_frame_geometry(poll_key.attachment()),
            None => {
                self.retire(CarrierFailure::Protocol);
                return true;
            }
        };
        let Some(projection) = self
            .projection
            .as_mut()
            .filter(|projection| &projection.key == poll_key.attachment())
        else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let result = projection.value.receive_for_geometry(&record, expected);
        let (settlement, frame, status) =
            match result {
                Ok(Some(feedback @ TerminalViewerAction::FrameApplied { .. })) => (
                    CausalSettlement::Feedback(feedback),
                    Some(FrameIdentity::new(poll_key.serial())),
                    NativeProjectionStatus::FrameAvailable,
                ),
                Ok(Some(feedback @ TerminalViewerAction::NeedSnapshot { .. })) => {
                    if self.state.as_mut().is_none_or(|state| {
                        !state.invalidate_projection_frame(poll_key.attachment())
                    }) {
                        self.retire(CarrierFailure::Protocol);
                        return true;
                    }
                    (
                        CausalSettlement::Feedback(feedback),
                        None,
                        NativeProjectionStatus::WaitingForFullFrame,
                    )
                }
                Ok(None) => {
                    if self.state.as_mut().is_none_or(|state| {
                        !state.invalidate_projection_frame(poll_key.attachment())
                    }) {
                        self.retire(CarrierFailure::Protocol);
                        return true;
                    }
                    (
                        CausalSettlement::NoFeedback,
                        None,
                        NativeProjectionStatus::Blocked,
                    )
                }
                Err(()) => {
                    self.retire(CarrierFailure::Protocol);
                    return true;
                }
                Ok(Some(_)) => {
                    self.retire(CarrierFailure::Protocol);
                    return true;
                }
            };
        self.projection_status = status;
        self.pending_draw = Some(PendingDraw {
            poll_key,
            settlement,
            frame,
            ordinary,
        });
        true
    }

    /// Reconciles the ordinary-Poll observation against the canonical mailbox: the UI
    /// serial is only bookkeeping, so it is cleared exactly when that serial is neither
    /// queued nor executing. A dropped or finished lease never blocks the next fair poll.
    fn reconcile_ordinary_poll(&mut self, mailbox: &Arc<CommandMailbox>) {
        let Some(serial) = self.ordinary_poll_serial else {
            return;
        };
        match mailbox.ordinary_poll_continues(serial) {
            Ok(true) => {}
            Ok(false) => self.ordinary_poll_serial = None,
            Err(closed) => self.retire(closed.failure()),
        }
    }

    /// Atomically captures a queued peer-publication hint into canonical refresh debt and
    /// drains one pending demand into a refresh flight. Busy/Full refusal is a local
    /// no-op: the debt stays pending and a later fair tick retries admission.
    fn drain_publication_refresh(&mut self, mailbox: &Arc<CommandMailbox>) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if let Err(closed) =
            mailbox.capture_publication_hint_for(state, NativeSessionState::note_publication_hint)
        {
            self.retire(closed.failure());
            return;
        }
        if !state.publication_debt_pending() {
            return;
        }
        let Some(deadline) = self.clock.now().checked_add(super::state::OPERATION_TOTAL) else {
            self.retire(CarrierFailure::Deadline);
            return;
        };
        let result = mailbox.try_enqueue_refresh_for(
            state,
            deadline,
            |state, generation| {
                state
                    .start_publication_drain(generation)
                    .then_some(())
                    .ok_or(ActionBuildError::Busy)
            },
            |_state, _result| {},
        );
        if let Err(error) = result {
            self.retire_for_admission(&error);
        }
    }

    fn schedule_poll(&mut self, mailbox: &Arc<CommandMailbox>) {
        if self.pending_draw.is_some() || self.ordinary_poll_serial.is_some() {
            return;
        }
        let Some(state) = self.state.as_ref() else {
            return;
        };
        if state.ready().is_none()
            || state.projection_key() != state.desired()
            || self
                .projection
                .as_ref()
                .is_none_or(|projection| Some(&projection.key) != state.desired())
        {
            return;
        }
        let Some(key) = state.desired().cloned() else {
            return;
        };
        if state.close_suppresses_attachment(&key) {
            return;
        }
        match mailbox.try_enqueue(QueuedCommand::Poll(Tagged::new(key, ()))) {
            Ok(serial) => self.ordinary_poll_serial = Some(serial),
            // Full and Busy are local no-ops: a canonical ordinary Poll may still be
            // queued or active even when the UI observation was cleared.
            Err(AdmissionError::Full | AdmissionError::Busy) => {}
            Err(AdmissionError::Closed(closed)) => self.retire(closed.failure()),
            Err(
                AdmissionError::BootstrapPending
                | AdmissionError::StaleAttachment
                | AdmissionError::AttachmentGenerationExhausted
                | AdmissionError::RefreshGenerationExhausted
                | AdmissionError::Build(_),
            ) => self.retire(CarrierFailure::Protocol),
        }
    }

    fn validate_state(&mut self) -> bool {
        let Some(state) = self.state.as_ref() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        if !state.invariant_holds() {
            self.retire(CarrierFailure::Protocol);
        } else if !state.close_status_only()
            && let Some(failure) = state.retired_failure()
        {
            self.retire(failure);
        }
        true
    }

    fn clear_projection(&mut self) {
        self.projection = None;
        self.projection_status = NativeProjectionStatus::Empty;
        self.pending_draw = None;
        self.ordinary_poll_serial = None;
    }

    fn handle_event_error(&mut self, error: EventError) -> bool {
        match error {
            EventError::ForwardExpired => false,
            EventError::Closed(closed) => {
                self.retire(closed.failure());
                true
            }
        }
    }

    fn retire(&mut self, failure: CarrierFailure) {
        #[cfg(target_os = "linux")]
        if self.retirement_failure.is_none() {
            self.retirement_poll_serial = self.ordinary_poll_serial;
        }
        if let Some(state) = &mut self.state {
            state.close_carrier_retired();
        }
        let failure = self
            .mailbox
            .as_ref()
            .map_or(failure, |mailbox| mailbox.close(failure).failure());
        if self.retirement_failure.is_none() {
            self.retirement_failure = Some(failure);
        }
        self.clear_projection();
    }
}

const fn create_precommit_notice(outcome: &CreatePrecommitOutcome) -> SessionNotice {
    match outcome {
        CreatePrecommitOutcome::NotSubmitted
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Rejected { .. })
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Denied)
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Unavailable { .. }) => {
            SessionNotice::CreateNotCreated
        }
    }
}

fn build_create(
    state: &mut NativeSessionState,
    generation: u64,
    request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    deadlines: OperationDeadlines,
) -> Result<CreateMarker, ActionBuildError> {
    if !state.invariant_holds() {
        return Err(ActionBuildError::Protocol);
    }
    let label_number = state
        .cache()
        .tabs()
        .len()
        .checked_add(1)
        .ok_or(ActionBuildError::Protocol)?;
    let label = WorkbenchLabel::parse(&label_number.to_string())
        .map_err(|_error| ActionBuildError::Protocol)?;
    let geometry = state.latest_geometry();
    let workbench_geometry = WorkbenchTerminalGeometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry.cell_pixel_width(),
        geometry.cell_pixel_height(),
    )
    .map_err(|_error| ActionBuildError::Protocol)?;
    let key = CreateOperationKey::new(request_id.clone(), generation);
    let operation = CreateOperation::new(
        key,
        state.cache().site_id().clone(),
        state.cache().revision(),
        state.cache().terminal_context_id().clone(),
        label.clone(),
        geometry,
        deadlines,
    )
    .with_origin(if state.phase_kind() == SessionPhaseKind::Empty {
        super::state::CreateOriginKind::Empty
    } else {
        super::state::CreateOriginKind::Ready
    });
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id,
        site_id: state.cache().site_id().clone(),
        expected_revision: state.cache().revision(),
        action: WorkbenchAction::TabCreate {
            terminal_context_id: state.cache().terminal_context_id().clone(),
            label,
            geometry: workbench_geometry,
        },
    };
    let request = WorkbenchTabCreateRequest::try_from(command)
        .map_err(|_command| ActionBuildError::Protocol)?;
    if !state.begin_create(operation.clone()) {
        return Err(ActionBuildError::Busy);
    }
    Ok(CreateMarker::new(request, operation))
}

fn build_switch(
    state: &mut NativeSessionState,
    action: ClientAction,
    generation: u64,
    deadlines: OperationDeadlines,
) -> Result<SwitchMarker, ActionBuildError> {
    if !state.invariant_holds() {
        return Err(ActionBuildError::Protocol);
    }
    let index = match state.plan_selection(action) {
        SelectionPlan::Busy => return Err(ActionBuildError::Busy),
        SelectionPlan::NoOp => return Err(ActionBuildError::NoOp),
        SelectionPlan::OutOfRange(position) => {
            return Err(ActionBuildError::OutOfRange(position));
        }
        SelectionPlan::Switch { index } => index,
    };
    let tab_id = state
        .tab(index)
        .map(|tab| tab.tab_id().clone())
        .ok_or(ActionBuildError::Protocol)?;
    let key = AttachmentKey::new(generation, tab_id);
    let target = state
        .target_for_switch(index, key, deadlines)
        .ok_or(ActionBuildError::Protocol)?;
    if !state.begin_switch(target.clone()) {
        return Err(ActionBuildError::Protocol);
    }
    Ok(SwitchMarker::new(target))
}

fn build_panel_switch(
    state: &mut NativeSessionState,
    generation: u64,
    direction: PanelDirection,
    deadlines: OperationDeadlines,
) -> Result<SwitchMarker, ActionBuildError> {
    if !state.invariant_holds() {
        return Err(ActionBuildError::Protocol);
    }
    let target = state
        .plan_panel_focus(direction, generation, deadlines)
        .ok_or(ActionBuildError::NoOp)?;
    state
        .begin_switch(target.clone())
        .then_some(SwitchMarker::new(target))
        .ok_or(ActionBuildError::Busy)
}

fn finish_action<Value>(
    state: &mut NativeSessionState,
    result: &Result<Value, AdmissionError<ActionBuildError>>,
) {
    match result {
        Ok(_) | Err(AdmissionError::Build(ActionBuildError::NoOp)) => state.clear_notice(),
        Err(AdmissionError::Build(ActionBuildError::OutOfRange(position))) => {
            state.set_notice(SessionNotice::NoTabAtPosition(*position));
        }
        Err(
            AdmissionError::BootstrapPending
            | AdmissionError::Busy
            | AdmissionError::Full
            | AdmissionError::StaleAttachment
            | AdmissionError::Build(ActionBuildError::Busy),
        ) => state.set_notice(SessionNotice::LocalBusy),
        Err(AdmissionError::Closed(_))
        | Err(AdmissionError::AttachmentGenerationExhausted)
        | Err(AdmissionError::RefreshGenerationExhausted)
        | Err(AdmissionError::Build(ActionBuildError::Protocol)) => {}
    }
}
