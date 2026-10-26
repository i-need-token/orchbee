mod action_menus;
mod agent_attention;
mod launch_profiles;
mod pane_graphics;
pub(in crate::workbench_client) mod settings;
mod sidebar_reorder;
mod tab_duplication;
mod tab_groups;

/// Display context for one local notice; it retains no executable text or launch authority.
pub(crate) enum NoticeContext {
    /// A missing name is the localized built-in System default label.
    LaunchProfile(Option<String>),
    ConfiguredCommand(String),
}
#[cfg(test)]
pub(in crate::workbench_client) use agent_attention::tests::focused_physical_agent_fixture;
mod agent_focus;
mod client_control;
mod client_layout;
use super::terminal_viewport::{self, ViewportWindow};
use super::{
    mailbox::{
        AdmissionError, CausalSettlement, CommandMailbox, CreateMarker, CreatePrecommitOutcome,
        EventError, MoveAdmissionError, MutationResult, QueuedCommand, RefreshSettlement,
        SwitchMarker, Tagged, UiReply, ViewportOutcome, WorkerEvent,
    },
    state::{
        AttachmentKey, CarrierFailure, ClientNotice, ClientPhaseKind,
        ClientTabView as StateTabView, ControllerMode, CreateOperation, CreateOperationKey,
        CreateProjectionDecision, CreateTerminalProvenance, FrameIdentity, NavigatorRefreshStatus,
        OperationDeadlines, PaneKey, PaneRef, PanelRequestKind, PendingCreateCommit,
        PendingMoveCommit, PendingPanelCommit, RefreshDecision, SelectionPlan,
        SetupEventDisposition, WorkbenchClientState, WorkbenchClientView as ClientStateView,
    },
    terminal_viewport::{
        TerminalViewport, ViewportEffect, ViewportNav, ViewportStep, decode_history_changed,
    },
};
use crate::{
    client_view::{ClientAction, ClientActionDisposition, PanelDirection},
    local_client::{
        LocalOperationClock,
        workbench::{WorkbenchTabCreatePrecommit, WorkbenchTabCreateRequest},
    },
    terminal_projection::TerminalProjection,
};
use orchbee_client_server_protocol::{ControlInput, Geometry, TerminalViewerAction};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchTabId, WorkbenchTabMoveDirection, WorkbenchTerminalGeometry,
};
use std::sync::Arc;

mod agent_board;
mod close_reconciliation;
mod control_takeover;
mod membership;
mod move_membership;
mod navigator;
mod panel;
mod panel_close;
mod presentations;
pub(crate) use presentations::DrawnPresentationTarget;
mod history_positions;
mod retained_frames;
/// Volatile Client view data carried across a targeted context surface replacement.
#[derive(Clone, Default)]
pub(crate) struct RetainedFrameBank {
    frames: retained_frames::RetainedFrames,
    history: history_positions::Bank,
}

#[cfg(test)]
impl From<retained_frames::RetainedFrames> for RetainedFrameBank {
    fn from(frames: retained_frames::RetainedFrames) -> Self {
        Self {
            frames,
            history: history_positions::Bank::default(),
        }
    }
}
mod retired_membership;
mod tab_close;
#[cfg(test)]
pub(in crate::workbench_client) mod tests;
mod workspace_close;
mod workspace_setup;
mod worktree_launch;
pub(crate) use workspace_close::{
    WorkspaceCloseConfirmationView, WorkspaceCloseStatus, WorkspaceCloseStatusView,
    WorkspaceCloseToken,
};
pub(crate) use workspace_setup::{
    LocationLabelStatus, LocationLabelTarget, SetupDraftView, SetupEdit, SetupStage, SetupStatus,
    SetupToken,
};
pub(crate) use workspace_setup::{WorkspaceCreationPhase, WorkspaceCreationPreview};
pub(crate) use worktree_launch::{
    WorktreeEdit, WorktreeMode, WorktreeNotice, WorktreeStage, WorktreeToken, WorktreeView,
};
mod copy_mode;
mod tab_rename;
mod terminal_effects;
mod terminal_selection;
#[cfg(test)]
pub(crate) use input_capture::WorktreeCaptureContext;
pub(crate) use input_capture::{CaptureContext, CaptureReadiness};
#[cfg(test)]
pub(crate) use input_capture::{ConfirmationCaptureContext, SetupCaptureContext};
pub(crate) use panel_close::{PanelCloseConfirmationToken, PanelCloseConfirmationView};
pub(crate) use tab_close::{
    CloseConfirmationKind, CloseConfirmationToken, CloseConfirmationView, CloseStatusKind,
    CloseStatusToken, CloseStatusView, DrawReceipt, ModalPresentation,
};
#[cfg(test)]
pub(crate) use tab_rename::RenameInputContext;
pub(crate) use tab_rename::{RenameDraftToken, RenameDraftView, RenameEdit};
mod create_membership;
mod direct_navigation;
mod startup_preference;
mod switch_membership;

#[cfg(test)]
pub(crate) use super::agent_board::AgentBoardInputContext;
pub(crate) use super::agent_board::{
    AgentBoardActualDraw, AgentBoardControl, AgentBoardControlTarget, AgentBoardDraw,
    AgentBoardEpisodeToken, AgentBoardRead, AgentBoardReadFailure, AgentBoardRow,
    AgentBoardRowTarget, AgentBoardSummary, AgentBoardToken, AgentBoardView, JumpFailure,
    PresentationPlacement,
};
pub(crate) use super::agent_board::{AgentStartCatalogStatus, AgentStartPhase};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DisplayPhase {
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
pub(crate) enum DisplayController {
    Controlling,
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DisplayProvenance {
    Started,
    Pending,
    Unavailable,
    Failed,
    OutcomeUnknown,
    BindingOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchClientNotice {
    Unsupported(crate::tui_capabilities::Capability),
    Settings(settings::Notice),
    MenuUnavailable,
    LocalBusy,
    NoTabAtPosition(u8),
    CreateNotCreated,
    CreateOrderConflict,
    DuplicatePending,
    DuplicateNotStarted,
    DuplicateOutcomeUnknown,
    MoveNotMoved,
    MoveOrderConflict,
    PanelNotApplied,
    RenameNotRenamed,
    CloseRequestNotApplied,
    WorkspaceCloseUnavailable,
    NoWorkspace,
    ConfirmationCancelled,
    DraftCancelled,
    ModalOwnsInput,
    AgentStart(super::agent_board::AgentStartPhase),
    HistoryRecovering,
    HistoryUnavailable,
    HistoryPositionLost,
    CopyModeUnavailable,
    CopyModeBusy,
    CopyModeInvalidated,
    CopyModeQueryFailed,
    CopyModeNoMatch,
    CopyModeSearchLimit,
    CopyModeNoSelection,
    SelectionEvicted,
    CopyFailed,
    CopyUnknown,
    BellFailed,
    BellUnknown,
    PositionChanged,
    PositionChangedEmpty,
    PositionLocationChanged,
    PositionLocationChangedEmpty,
    PositionUnavailable,
    PopupUnavailable,
    CommandAccepted,
    CommandFailed,
    CommandUnknown,
    CommandRefused(orchbee_workbench_protocol::ConfiguredCommandFailure),
    LaunchProfileSelected,
    LaunchProfileUnavailable,
    LinkOpened,
    LinkHandled,
    LinkNoLink,
    LinkBusy,
    LinkFailed,
    LinkUnknown,
}

const fn link_notice(notice: crate::terminal_link_activation::Notice) -> WorkbenchClientNotice {
    match notice {
        crate::terminal_link_activation::Notice::Unsupported => {
            WorkbenchClientNotice::Unsupported(crate::tui_capabilities::Capability::Browser)
        }
        crate::terminal_link_activation::Notice::Opened => WorkbenchClientNotice::LinkOpened,
        crate::terminal_link_activation::Notice::Handled => WorkbenchClientNotice::LinkHandled,
        crate::terminal_link_activation::Notice::NoLink => WorkbenchClientNotice::LinkNoLink,
        crate::terminal_link_activation::Notice::Failed => WorkbenchClientNotice::LinkFailed,
        crate::terminal_link_activation::Notice::Unknown => WorkbenchClientNotice::LinkUnknown,
    }
}

/// What one pane's retained-history surface paints right now. `Window`
/// hands out a row window the core re-validated against current geometry
/// and facts; the other cases overlay their own indicator and never hand
/// out stale row data.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ViewportSurface<'a> {
    Window {
        window: &'a ViewportWindow,
        /// `(start_row, total_rows)` re-validated at the same instant the
        /// window was fenced — the only honest position source.
        position: Option<(u64, u64)>,
    },
    Entering,
    AwaitingWindow,
    Recovering,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RefreshStatus {
    Refreshing,
    Success(u64),
    Failure(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientProjectionStatus {
    Empty,
    FrameAvailable,
    WaitingForFullFrame,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DisplayRetirement {
    Unavailable,
    OutcomeUnknown,
}

#[derive(Clone, Copy)]
pub(crate) struct ClientTabView<'a> {
    inner: StateTabView<'a>,
    observed_title: Option<&'a str>,
    resize: Option<&'a crate::tui_layout::ResizeProjection>,
}

impl<'a> ClientTabView<'a> {
    pub(crate) fn group(&self) -> Option<&'a orchbee_workbench_protocol::TabGroup> {
        self.inner.group()
    }

    pub(crate) fn pinned(&self) -> bool {
        self.inner.pinned()
    }

    pub(crate) fn group_label(&self) -> Option<&'a str> {
        self.inner.group_label()
    }

    /// Resolve this Client's split weight against exact ordered slot identities.
    pub(crate) fn split_ratio(
        &self,
        split: &orchbee_workbench_protocol::WorkbenchSplitId,
        first: &orchbee_workbench_protocol::WorkbenchPanelSlotId,
        second: &orchbee_workbench_protocol::WorkbenchPanelSlotId,
    ) -> orchbee_workbench_protocol::ClientSplitRatio {
        let canonical = self.inner.split_ratio(split, first, second);
        self.resize.map_or(canonical, |resize| {
            resize.ratio(self.tab_id(), split, first, second, canonical)
        })
    }

    pub(crate) const fn position(&self) -> usize {
        self.inner.position()
    }

    pub(crate) const fn active(&self) -> bool {
        self.inner.active()
    }

    pub(crate) fn tab_label(&self) -> std::borrow::Cow<'a, str> {
        self.inner.tab_label()
    }

    /// Uses observed terminal titles for automatic chrome; fixed names remain exact.
    pub(crate) fn display_label(
        &self,
        locale: crate::localization::Locale,
    ) -> std::borrow::Cow<'a, str> {
        if self.inner.automatic_label() {
            self.observed_title.map_or_else(
                || locale.automatic_tab_label(self.position()).into(),
                std::borrow::Cow::Borrowed,
            )
        } else {
            self.tab_label()
        }
    }

    pub(crate) fn panel_count(&self) -> usize {
        self.inner.panel_count()
    }

    pub(crate) fn panel_tree(&self) -> &'a super::ClientPanelTree {
        self.inner.panel_tree()
    }

    pub(crate) fn active_panel(&self) -> Option<usize> {
        self.inner.active_panel()
    }

    pub(crate) fn panel(&self, index: usize) -> Option<&'a super::ClientPanel> {
        self.inner.panel(index)
    }

    /// The tab's stable identity — recorded into pointer hit regions so a
    /// click resolves the entry, never a stale position.
    pub(crate) fn tab_id(&self) -> &'a WorkbenchTabId {
        self.inner.tab_id()
    }
}

pub(crate) struct WorkbenchClientView<'a> {
    resize: Option<&'a crate::tui_layout::ResizeProjection>,
    settings: Option<&'a settings::Coordinator>,
    completion_reminder: Option<&'a super::completion_reminders::Reminder>,
    menu: Option<&'a action_menus::Coordinator>,
    state: Option<ClientStateView<'a>>,
    panes: &'a [KeyedProjection],
    retained_frames: Option<retained_frames::RetainedView<'a>>,
    focus: Option<&'a WorkbenchPanelId>,
    projection_status: ClientProjectionStatus,
    retirement: Option<DisplayRetirement>,
    retirement_requested: bool,
    board: Option<AgentBoardView<'a>>,
    worktree: Option<&'a crate::workbench_client::state::worktree_launch::Launcher>,
    board_summary: Option<&'a AgentBoardSummary>,
    copy_mode: Option<&'a copy_mode::Coordinator>,
    attention: Option<&'a super::agent_attention::AgentAttention>,
    reports: Option<&'a super::panel_reports::PanelReports>,
    graphics: Option<&'a super::pane_graphics::Driver>,
    graphics_surface: Option<crate::tui_graphics::Surface>,
    popup_failed: bool,
    command_notice: Option<WorkbenchClientNotice>,
    notice_context: Option<&'a NoticeContext>,
    popup: Option<&'a super::independent_popup::Driver>,
}

/// Bounded launch facts for a failed member; no command arguments or environment.
#[derive(Clone, Copy)]
pub(crate) struct LaunchFailure<'a> {
    pub(crate) profile: LaunchProfile<'a>,
    pub(crate) executable: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LaunchProfile<'a> {
    Named(&'a str),
    SystemDefault,
    Unknown,
}

impl<'a> WorkbenchClientView<'a> {
    pub(crate) fn launch_failure(
        &self,
        panel: &'a super::ClientPanel,
    ) -> Option<LaunchFailure<'a>> {
        let state = self.state.as_ref()?;
        let observed = state.selected_presentation(panel).launch_failure();
        let failed = observed.is_some()
            || state.pane_by_panel(panel.panel_id()).is_some_and(|record| {
                matches!(record.provenance(), CreateTerminalProvenance::Failed(_))
            })
            || (self.focus == Some(panel.panel_id())
                && self.selected_provenance() == Some(DisplayProvenance::Failed));
        if !failed || self.retirement.is_some() {
            return None;
        }
        // This Client's captured description names the profile; the Server fact names only
        // the executable, so a Tab created elsewhere never claims a profile identity.
        Some(match (state.launch_description(panel), observed) {
            (Some(description), _) => LaunchFailure {
                profile: description
                    .profile_name
                    .as_deref()
                    .map_or(LaunchProfile::SystemDefault, LaunchProfile::Named),
                executable: description.executable.as_deref(),
            },
            (None, Some(observed)) => LaunchFailure {
                profile: LaunchProfile::Unknown,
                executable: observed.executable.as_deref(),
            },
            (None, None) => LaunchFailure {
                profile: LaunchProfile::Unknown,
                executable: None,
            },
        })
    }
    /// A shell-local projection never changes the cache or canonical layout inspection.
    pub(crate) fn set_resize_projection(
        &mut self,
        resize: &'a crate::tui_layout::ResizeProjection,
    ) {
        self.resize = Some(resize);
    }

    pub(crate) fn sidebar_surface(
        &self,
        canonical: crate::client_view::SidebarSurface,
    ) -> crate::client_view::SidebarSurface {
        self.resize
            .map_or(canonical, |resize| resize.sidebar(canonical))
    }
    pub(crate) fn selected_tab_ids(&self) -> Vec<orchbee_workbench_protocol::WorkbenchTabId> {
        self.state
            .as_ref()
            .map_or_else(Vec::new, ClientStateView::selected_tab_ids)
    }
    pub(crate) fn group_collapsed(
        &self,
        id: &orchbee_workbench_protocol::WorkbenchTabGroupId,
    ) -> bool {
        self.state
            .as_ref()
            .is_some_and(|state| state.group_collapsed(id))
    }

    /// Immutable selected layout identity for a divider painted from this view.
    pub(crate) fn divider_source(&self) -> Option<crate::tui_layout::DividerSource> {
        self.state.as_ref()?.divider_source()
    }
    pub(super) fn window_title_context(&self) -> orchbee_workbench_protocol::WindowTitleContext {
        let workspace = self
            .active_context_position()
            .and_then(|position| position.checked_sub(1))
            .and_then(|index| self.context_at(index))
            .and_then(|context| context.workspace_label())
            .unwrap_or("");
        let tab = self
            .active_position()
            .and_then(|position| position.checked_sub(1))
            .and_then(|index| self.tab(index));
        let label = tab.as_ref().map(|tab| tab.tab_label()).unwrap_or_default();
        let pane = tab.as_ref().and_then(|tab| {
            self.focus_panel_id().and_then(|id| {
                (0..tab.panel_count())
                    .filter_map(|index| tab.panel(index))
                    .find(|panel| panel.panel_id() == id)
            })
        });
        let manual = pane
            .map(|panel| self.selected_presentation(panel))
            .and_then(|member| member.custom_label())
            .unwrap_or("");
        let title = self
            .focus_panel_id()
            .filter(|panel| {
                self.retirement.is_none()
                    && self.pane_projection_status(panel) == ClientProjectionStatus::FrameAvailable
            })
            .and_then(|panel| self.pane_projection(panel))
            .and_then(|projection| projection.frame())
            .and_then(|frame| frame.title.as_deref())
            .and_then(crate::terminal_title::display)
            .unwrap_or("");
        orchbee_workbench_protocol::WindowTitleContext::new(workspace, &label, manual, title)
    }

    pub(crate) fn panel_report(&self, panel: &super::ClientPanel) -> super::PanelReportView<'_> {
        if self.retirement.is_some() {
            return super::PanelReportView::Unavailable;
        }
        self.reports.map_or(super::PanelReportView::Pending, |r| {
            r.view(panel, self.selected_presentation(panel))
        })
    }
    pub(crate) fn context_count(&self) -> usize {
        self.state
            .as_ref()
            .map_or(0, ClientStateView::context_count)
    }
    pub(crate) fn navigation_session(&self) -> Option<&orchbee_workbench_protocol::SessionId> {
        self.state.as_ref().map(ClientStateView::navigation_session)
    }

    pub(crate) fn active_context_position(&self) -> Option<usize> {
        self.state.as_ref()?.active_context_position()
    }

    pub(crate) fn context_at(&self, index: usize) -> Option<super::state::ClientContextView<'a>> {
        self.state.as_ref()?.context_at(index)
    }

    pub(crate) const fn phase(&self) -> DisplayPhase {
        let Some(state) = self.state.as_ref() else {
            return DisplayPhase::Starting;
        };
        match state.phase() {
            ClientPhaseKind::Empty => DisplayPhase::Empty,
            ClientPhaseKind::Starting => DisplayPhase::Starting,
            ClientPhaseKind::Ready => DisplayPhase::Ready,
            ClientPhaseKind::Creating => DisplayPhase::Creating,
            ClientPhaseKind::CreateCommittedNeedDetach => DisplayPhase::CreateCommittedNeedDetach,
            ClientPhaseKind::ProjectionUnavailable => DisplayPhase::ProjectionUnavailable,
            ClientPhaseKind::MoveProjectionUnavailable => DisplayPhase::MoveProjectionUnavailable,
            ClientPhaseKind::PanelProjectionUnavailable => DisplayPhase::PanelProjectionUnavailable,
            ClientPhaseKind::RenameProjectionUnavailable => {
                DisplayPhase::RenameProjectionUnavailable
            }
            ClientPhaseKind::Switching => DisplayPhase::Switching,
            ClientPhaseKind::SelectedWithoutAttachment => DisplayPhase::SelectedWithoutAttachment,
            ClientPhaseKind::RetryableAttachUnavailable => DisplayPhase::RetryableAttachUnavailable,
            ClientPhaseKind::Retired => DisplayPhase::Retired,
        }
    }

    /// The selected empty landing is absent from Workspace navigation.
    pub(crate) fn no_current_workspace(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(ClientStateView::no_current_workspace)
    }

    pub(crate) fn tab_count(&self) -> usize {
        self.state.as_ref().map_or(0, ClientStateView::tab_count)
    }

    pub(crate) fn tab(&self, index: usize) -> Option<ClientTabView<'a>> {
        self.state.as_ref()?.tab(index).map(|inner| ClientTabView {
            observed_title: inner
                .active_panel()
                .or(Some(0))
                .and_then(|index| inner.panel(index))
                .and_then(|panel| {
                    let member = self.selected_presentation(panel);
                    member
                        .custom_label()
                        .or_else(|| {
                            (self.retirement.is_none()
                                && self.pane_projection_status(panel.panel_id())
                                    == ClientProjectionStatus::FrameAvailable)
                                .then(|| self.pane_projection(panel.panel_id()))
                                .flatten()
                                .and_then(|projection| projection.frame())
                                .and_then(|frame| frame.title.as_deref())
                                .and_then(crate::terminal_title::display)
                        })
                        .or_else(|| {
                            (self.retirement.is_none()
                                && (!inner.active()
                                    || matches!(
                                        self.pane_projection_status(panel.panel_id()),
                                        ClientProjectionStatus::Empty
                                            | ClientProjectionStatus::WaitingForFullFrame
                                    )))
                            .then(|| {
                                self.retained_frames.as_ref()?.title(
                                    inner.tab_id(),
                                    panel,
                                    self.pane_projection(panel.panel_id())
                                        .map(TerminalProjection::epoch),
                                )
                            })
                            .flatten()
                        })
                        .or_else(|| {
                            self.state
                                .as_ref()?
                                .launch_description(panel)?
                                .executable
                                .as_deref()
                                .and_then(|program| program.rsplit(['/', '\\']).next())
                        })
                        .filter(|label| !label.trim().is_empty())
                }),
            inner,
            resize: self.resize,
        })
    }

    pub(crate) fn active_position(&self) -> Option<usize> {
        self.state.as_ref()?.active_position()
    }

    pub(crate) fn controller(&self) -> Option<DisplayController> {
        self.state
            .as_ref()?
            .controller_mode()
            .map(|mode| match mode {
                ControllerMode::Controlling(_) => DisplayController::Controlling,
                ControllerMode::ReadOnly => DisplayController::ReadOnly,
            })
    }

    pub(crate) fn selected_provenance(&self) -> Option<DisplayProvenance> {
        let state = self.state.as_ref()?;
        // The Server's committed failure also explains a Tab created by another Client.
        if state.selected_launch_failure().is_some() {
            return Some(DisplayProvenance::Failed);
        }
        state
            .selected_provenance()
            .map(|provenance| match provenance {
                CreateTerminalProvenance::Started => DisplayProvenance::Started,
                CreateTerminalProvenance::Pending => DisplayProvenance::Pending,
                CreateTerminalProvenance::Unavailable(_) => DisplayProvenance::Unavailable,
                CreateTerminalProvenance::Failed(_) => DisplayProvenance::Failed,
                CreateTerminalProvenance::OutcomeUnknown => DisplayProvenance::OutcomeUnknown,
                CreateTerminalProvenance::BindingOnly => DisplayProvenance::BindingOnly,
            })
    }

    pub(crate) fn notice(&self) -> Option<WorkbenchClientNotice> {
        if self.popup_failed {
            return Some(WorkbenchClientNotice::PopupUnavailable);
        }
        if self.command_notice.is_some() {
            return self.command_notice;
        }
        self.state.as_ref()?.notice().map(|notice| match notice {
            ClientNotice::LocalBusy => WorkbenchClientNotice::LocalBusy,
            ClientNotice::NoTabAtPosition(position) => {
                WorkbenchClientNotice::NoTabAtPosition(position)
            }
            ClientNotice::CreateNotCreated => WorkbenchClientNotice::CreateNotCreated,
            ClientNotice::CreateOrderConflict => WorkbenchClientNotice::CreateOrderConflict,
            ClientNotice::DuplicatePending => WorkbenchClientNotice::DuplicatePending,
            ClientNotice::DuplicateNotStarted => WorkbenchClientNotice::DuplicateNotStarted,
            ClientNotice::DuplicateOutcomeUnknown => WorkbenchClientNotice::DuplicateOutcomeUnknown,
            ClientNotice::MoveNotMoved => WorkbenchClientNotice::MoveNotMoved,
            ClientNotice::MoveOrderConflict => WorkbenchClientNotice::MoveOrderConflict,
            ClientNotice::PanelNotApplied => WorkbenchClientNotice::PanelNotApplied,
            ClientNotice::RenameNotRenamed => WorkbenchClientNotice::RenameNotRenamed,
            ClientNotice::CloseRequestNotApplied => WorkbenchClientNotice::CloseRequestNotApplied,
            ClientNotice::WorkspaceCloseUnavailable => {
                WorkbenchClientNotice::WorkspaceCloseUnavailable
            }
            ClientNotice::NoWorkspace => WorkbenchClientNotice::NoWorkspace,
            ClientNotice::ConfirmationCancelled => WorkbenchClientNotice::ConfirmationCancelled,
            ClientNotice::DraftCancelled => WorkbenchClientNotice::DraftCancelled,
            ClientNotice::ModalOwnsInput => WorkbenchClientNotice::ModalOwnsInput,
            ClientNotice::AgentStart(phase) => WorkbenchClientNotice::AgentStart(phase),
            ClientNotice::HistoryRecovering => WorkbenchClientNotice::HistoryRecovering,
            ClientNotice::HistoryUnavailable => WorkbenchClientNotice::HistoryUnavailable,
            ClientNotice::HistoryPositionLost => WorkbenchClientNotice::HistoryPositionLost,
            ClientNotice::CopyModeUnavailable => WorkbenchClientNotice::CopyModeUnavailable,
            ClientNotice::CopyModeBusy => WorkbenchClientNotice::CopyModeBusy,
            ClientNotice::CopyModeInvalidated => WorkbenchClientNotice::CopyModeInvalidated,
            ClientNotice::CopyModeQueryFailed => WorkbenchClientNotice::CopyModeQueryFailed,
            ClientNotice::CopyModeNoMatch => WorkbenchClientNotice::CopyModeNoMatch,
            ClientNotice::CopyModeSearchLimit => WorkbenchClientNotice::CopyModeSearchLimit,
            ClientNotice::CopyModeNoSelection => WorkbenchClientNotice::CopyModeNoSelection,
            ClientNotice::SelectionEvicted => WorkbenchClientNotice::SelectionEvicted,
            ClientNotice::Unsupported(capability) => WorkbenchClientNotice::Unsupported(capability),
            ClientNotice::CopyFailed => WorkbenchClientNotice::CopyFailed,
            ClientNotice::CopyUnknown => WorkbenchClientNotice::CopyUnknown,
            ClientNotice::BellFailed => WorkbenchClientNotice::BellFailed,
            ClientNotice::BellUnknown => WorkbenchClientNotice::BellUnknown,
            ClientNotice::PositionChanged => WorkbenchClientNotice::PositionChanged,
            ClientNotice::PositionChangedEmpty => WorkbenchClientNotice::PositionChangedEmpty,
            ClientNotice::PositionLocationChanged => WorkbenchClientNotice::PositionLocationChanged,
            ClientNotice::PositionLocationChangedEmpty => {
                WorkbenchClientNotice::PositionLocationChangedEmpty
            }
            ClientNotice::PositionUnavailable => WorkbenchClientNotice::PositionUnavailable,
        })
    }

    /// Only the matching notice may use this retained operation context.
    pub(crate) fn notice_context(&self) -> Option<&NoticeContext> {
        match (self.notice()?, self.notice_context?) {
            (
                WorkbenchClientNotice::CommandAccepted
                | WorkbenchClientNotice::CommandFailed
                | WorkbenchClientNotice::CommandUnknown
                | WorkbenchClientNotice::CommandRefused(_),
                context @ NoticeContext::ConfiguredCommand(_),
            )
            | (
                WorkbenchClientNotice::LaunchProfileSelected
                | WorkbenchClientNotice::LaunchProfileUnavailable,
                context @ NoticeContext::LaunchProfile(_),
            ) => Some(context),
            _ => None,
        }
    }

    pub(crate) fn refresh_status(&self) -> Option<RefreshStatus> {
        self.state
            .as_ref()?
            .refresh_status()
            .map(|status| match status {
                NavigatorRefreshStatus::Refreshing => RefreshStatus::Refreshing,
                NavigatorRefreshStatus::Success(revision) => {
                    RefreshStatus::Success(revision.value())
                }
                NavigatorRefreshStatus::Failure(revision) => {
                    RefreshStatus::Failure(revision.value())
                }
            })
    }

    pub(crate) const fn projection_status(&self) -> ClientProjectionStatus {
        self.projection_status
    }

    /// The focus pane's projection — kept for single-pane consumers; paned
    /// rendering uses [`Self::pane_projection`] per leaf.
    pub(crate) fn projection(&self) -> Option<&'a TerminalProjection> {
        self.focus.and_then(|panel| self.pane_projection(panel))
    }

    pub(crate) fn terminal_end(
        &self,
        panel: &WorkbenchPanelId,
    ) -> Option<crate::terminal_projection::TerminalEnd> {
        self.pane_projection(panel)?.end()
    }

    /// The live projection installed for one panel's exact pane incarnation,
    /// or `None` while that pane has no viewer content yet.
    pub(crate) fn pane_projection(
        &self,
        panel: &WorkbenchPanelId,
    ) -> Option<&'a TerminalProjection> {
        let state = self.state.as_ref()?;
        let record = state.pane_by_panel(panel)?;
        self.panes
            .iter()
            .find(|projection| projection.key == record.pane_ref())
            .map(|projection| &projection.value)
    }

    /// Retained live pixels for this exact canonical terminal while a new
    /// viewer is connecting. The renderer must keep cursor and interaction off.
    pub(crate) fn retained_pane_frame(
        &self,
        panel: &WorkbenchPanelId,
        columns: u16,
        rows: u16,
    ) -> Option<&'a orchbee_terminal_protocol::TerminalTextFrame> {
        if self.retirement.is_some()
            || !matches!(
                self.pane_projection_status(panel),
                ClientProjectionStatus::Empty | ClientProjectionStatus::WaitingForFullFrame
            )
        {
            return None;
        }
        let frame = self.retained_frames.as_ref()?.frame(panel, columns, rows)?;
        if self
            .pane_projection(panel)
            .is_some_and(|live| *live.epoch() != frame.route.host_epoch)
        {
            return None;
        }
        Some(frame)
    }

    /// Host-owned endpoints for this exact panel incarnation, projected by the painter.
    pub(crate) fn pane_selection(
        &self,
        panel: &WorkbenchPanelId,
    ) -> Option<&'a orchbee_terminal_protocol::SelectionChangedState> {
        let record = self.state.as_ref()?.pane_by_panel(panel)?;
        self.panes
            .iter()
            .find(|projection| projection.key == record.pane_ref())?
            .selection
            .as_ref()
            .map(super::terminal_selection::Driver::selection)
    }

    /// The exact live incarnation bound to one panel — anchor generation,
    /// stable panel, and incarnation — or `None` while the panel has no
    /// record. Pane hit regions record this so a click can never migrate
    /// onto a re-minted record of the same logical pane.
    pub(crate) fn pane_ref_for_panel(&self, panel: &WorkbenchPanelId) -> Option<PaneRef> {
        self.state
            .as_ref()?
            .pane_by_panel(panel)
            .map(|record| record.pane_ref())
    }

    /// The draw status of one panel's projection — `Empty` when the pane has
    /// no installed projection at all.
    pub(crate) fn pane_projection_status(
        &self,
        panel: &WorkbenchPanelId,
    ) -> ClientProjectionStatus {
        let Some(state) = self.state.as_ref() else {
            return ClientProjectionStatus::Empty;
        };
        let Some(record) = state.pane_by_panel(panel) else {
            return ClientProjectionStatus::Empty;
        };
        self.panes
            .iter()
            .find(|projection| projection.key == record.pane_ref())
            .map_or(ClientProjectionStatus::Empty, |projection| {
                projection.status
            })
    }

    /// One panel's retained-history surface, or `None` while its pane is
    /// Live. The returned window is already fenced to the pane's current
    /// generation and column geometry — the renderer only clips it to the
    /// leaf bounds.
    pub(crate) fn pane_viewport(&self, panel: &WorkbenchPanelId) -> Option<ViewportSurface<'_>> {
        let state = self.state.as_ref()?;
        let record = state.pane_by_panel(panel)?;
        let projection = self
            .panes
            .iter()
            .find(|projection| projection.key == record.pane_ref())?;
        if projection.tail.is_some() {
            return None;
        }
        let viewport = projection.viewport.as_ref()?;
        Some(match viewport.surface() {
            terminal_viewport::ViewportSurface::Window(window) => ViewportSurface::Window {
                window,
                position: viewport.position(),
            },
            terminal_viewport::ViewportSurface::Entering => ViewportSurface::Entering,
            terminal_viewport::ViewportSurface::AwaitingWindow => ViewportSurface::AwaitingWindow,
            terminal_viewport::ViewportSurface::Recovering => ViewportSurface::Recovering,
        })
    }

    /// Capture the exact canonical member and terminal row surface this view
    /// would paint for one panel. Loading indicators and unbound panes expose
    /// no link source.
    pub(crate) fn painted_content_source(
        &self,
        panel_id: &WorkbenchPanelId,
    ) -> Option<crate::terminal_content::ContentSource> {
        self.content_source(panel_id, true)
    }

    /// Copy owns a local view of the complete admitted grid, independent of the live crop.
    pub(crate) fn copy_content_source(
        &self,
        panel_id: &WorkbenchPanelId,
    ) -> Option<crate::terminal_content::ContentSource> {
        self.content_source(panel_id, false)
    }

    fn content_source(
        &self,
        panel_id: &WorkbenchPanelId,
        crop_live: bool,
    ) -> Option<crate::terminal_content::ContentSource> {
        use crate::terminal_content::{ContentSource, PaintedRows, SnapshotMember, TerminalScreen};

        let state = self.state.as_ref()?;
        let record = state.pane_by_panel(panel_id)?;
        let pane = record.pane_ref();
        let tab = self.tab(self.active_position()?.checked_sub(1)?)?;
        if tab.tab_id() != pane.anchor().tab_id() {
            return None;
        }
        let panel = (0..tab.panel_count())
            .filter_map(|index| tab.panel(index))
            .find(|panel| panel.panel_id() == panel_id)?;
        let selected = self.selected_presentation(panel);
        let binding = selected.binding()?;
        if record.binding() != binding {
            return None;
        }
        let projection = self.pane_projection(panel_id)?;
        if self
            .panes
            .iter()
            .any(|pane| pane.key == record.pane_ref() && pane.tail.is_some())
            && self.pane_tail_window(panel_id).is_none()
        {
            return None;
        }
        let host_terminal =
            orchbee_client_server_protocol::HostTerminalId::parse(projection.host_terminal_id()?)
                .ok()?;
        let (screen, rows) = match self.pane_viewport(panel_id).or_else(|| {
            self.pane_tail_window(panel_id)
                .map(|(window, _)| ViewportSurface::Window {
                    window,
                    position: None,
                })
        }) {
            Some(ViewportSurface::Window { window, .. }) => (
                TerminalScreen::Primary,
                PaintedRows {
                    attachment: window.route.attachment_id,
                    host_epoch: window.route.host_epoch,
                    sequence: window.route.sequence,
                    generation: window.route.generation,
                    revision: window.route.revision,
                    columns: window.route.columns,
                    start_column: 0,
                    total_rows: window.route.total_rows,
                    start_row: window.route.start_row,
                    row_count: window.route.row_count,
                    history: true,
                },
            ),
            Some(
                ViewportSurface::Entering
                | ViewportSurface::AwaitingWindow
                | ViewportSurface::Recovering,
            ) => return None,
            None => {
                let frame = projection.frame()?;
                let mut rows = PaintedRows::live(frame)?;
                if crop_live && let Some(crop) = self.pane_live_view(panel_id) {
                    rows.start_column = crop.column;
                    rows.start_row = rows.start_row.checked_add(u64::from(crop.row))?;
                    rows.row_count = crop.rows;
                }
                (TerminalScreen::from_frame(frame), rows)
            }
        };
        Some(ContentSource {
            pane,
            member: SnapshotMember {
                session: state.session_id().clone(),
                context: state.terminal_context_id()?.clone(),
                tab: tab.tab_id().clone(),
                panel: panel_id.clone(),
                presentation: selected.presentation_id().clone(),
                worker: binding.worker_id().clone(),
                terminal: binding.terminal_id().clone(),
            },
            host_terminal,
            screen,
            rows,
        })
    }

    pub(crate) fn painted_link_source(
        &self,
        panel_id: &WorkbenchPanelId,
    ) -> Option<crate::terminal_link_activation::PaintedLinkSource> {
        self.painted_content_source(panel_id)
    }

    /// The same local live rectangle used by paint, selection and content inspection.
    pub(crate) fn pane_live_view(
        &self,
        panel: &WorkbenchPanelId,
    ) -> Option<crate::terminal_live_view::LiveView> {
        let record = self.state.as_ref()?.pane_by_panel(panel)?;
        let projection = self.panes.iter().find(|p| p.key == record.pane_ref())?;
        if projection.viewport.is_some() {
            return None;
        }
        let frame = projection.value.frame()?;
        let geometry = record.observed_geometry();
        Some(crate::terminal_live_view::LiveView::new(
            frame,
            geometry.columns(),
            geometry.rows(),
        ))
    }

    /// The panel identity of the focus pane — the leaf that takes input,
    /// resize and the focused-frame chrome.
    pub(crate) const fn focus_panel_id(&self) -> Option<&'a WorkbenchPanelId> {
        self.focus
    }
    pub(crate) fn focused_member(&self) -> Option<super::ClientMemberTarget> {
        if self.retirement.is_some() {
            return None;
        }
        let tab = self.tab(self.active_position()?.checked_sub(1)?)?;
        let panel = (0..tab.panel_count())
            .filter_map(|i| tab.panel(i))
            .find(|p| Some(p.panel_id()) == self.focus_panel_id())?;
        Some(super::ClientMemberTarget {
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: self.selected_presentation(panel).presentation_id().clone(),
        })
    }

    /// The active Tab renders zoomed: the focused leaf draws across the
    /// whole pane content area while hidden leaves stay attached but
    /// undrawn. Volatile Client-local Client state.
    pub(crate) fn zoom_active(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(ClientStateView::zoom_active)
    }

    pub(crate) const fn retirement(&self) -> Option<DisplayRetirement> {
        self.retirement
    }

    /// The Agent board's read handle — `Some` only while the board is
    /// open, so a closed board never paints and never claims hits.
    pub(crate) const fn agent_board(&self) -> Option<&AgentBoardView<'a>> {
        self.board.as_ref()
    }

    /// The sidebar's summary of the last completed Agent read — `None`
    /// when no read has completed, never an implied zero.
    pub(crate) const fn agent_board_summary(&self) -> Option<AgentBoardSummary> {
        self.board_summary.copied()
    }

    pub(crate) fn copy_mode(&self) -> Option<crate::terminal_copy_mode::CopyModeView<'a>> {
        self.copy_mode.and_then(copy_mode::Coordinator::view)
    }
}

#[cfg(test)]
impl<'a> WorkbenchClientView<'a> {
    /// Empty, unobstructed Native surface for consumers whose test owns no Client state.
    pub(crate) fn notification_test_view() -> Self {
        Self {
            worktree: None,
            menu: None,
            settings: None,
            completion_reminder: None,
            state: None,
            resize: None,
            panes: &[],
            retained_frames: None,
            focus: None,
            projection_status: ClientProjectionStatus::Empty,
            retirement: None,
            retirement_requested: false,
            board: None,
            board_summary: None,
            copy_mode: None,
            attention: None,
            reports: None,
            graphics: None,
            graphics_surface: None,
            popup_failed: false,
            command_notice: None,
            notice_context: None,
            popup: None,
        }
    }

    /// Test-only view over a bare Client state: the caller supplies the
    /// live projections and the focus draw status the paint pass would
    /// report — the exact shape `pane_geometries`, `active_pane_area`, and
    /// the renderer consume.
    pub(super) fn test_view(
        state: &'a WorkbenchClientState,
        panes: &'a [KeyedProjection],
        projection_status: ClientProjectionStatus,
    ) -> Self {
        Self {
            worktree: None,
            menu: None,
            settings: None,
            completion_reminder: None,
            state: Some(state.view()),
            resize: None,
            panes,
            retained_frames: None,
            focus: state.focus_panel_id(),
            projection_status,
            retirement: None,
            retirement_requested: false,
            board: None,
            board_summary: None,
            copy_mode: None,
            attention: None,
            reports: None,
            graphics: None,
            graphics_surface: None,
            popup_failed: false,
            command_notice: None,
            notice_context: None,
            popup: None,
        }
    }
}

/// One pane's installed projection plus its draw status, keyed by the exact
/// incarnation the `Attached` event settled.
pub(crate) struct KeyedProjection {
    key: PaneRef,
    value: TerminalProjection,
    status: ClientProjectionStatus,
    /// The retained-row lease for manual history or a verified automatic tail window.
    /// No core exists before its `Enter` step is admitted; `tail` identifies live-input use.
    viewport: Option<TerminalViewport>,
    /// Automatic tail holds the same history lease, but never captures ordinary input.
    tail: Option<super::terminal_tail::Target>,
    tail_attempted: Option<super::terminal_tail::Target>,
    /// Strict per-incarnation kind-53 sequence cursor: the next
    /// `HistoryChanged` must exceed this or the stream is malformed.
    history_sequence: u64,
    /// Per-incarnation history read watermark: the highest `request_sequence`
    /// ever staged for this attachment. The host's
    /// `last_row_request_sequence` is attachment-scoped and survives
    /// viewport episodes, so a re-entered history episode resumes above it;
    /// a re-minted `KeyedProjection` starts at 0 because a new attachment
    /// legitimately resets the space.
    read_watermark: u64,
    selection: Option<super::terminal_selection::Driver>,
    effect_stream: super::terminal_effects::Stream,
}

#[cfg(test)]
impl KeyedProjection {
    /// Test-only record for [`WorkbenchClientView::test_view`]: a live
    /// projection bound to an exact pane incarnation.
    pub(super) fn for_test(
        key: PaneRef,
        value: TerminalProjection,
        status: ClientProjectionStatus,
    ) -> Self {
        Self {
            effect_stream: super::terminal_effects::Stream::new(
                *value.attachment(),
                *value.epoch(),
            ),
            key,
            value,
            status,
            viewport: None,
            tail: None,
            tail_attempted: None,
            history_sequence: 0,
            read_watermark: 0,
            selection: None,
        }
    }
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

pub(super) struct WorkbenchClientUi {
    capabilities: crate::tui_capabilities::Capabilities,
    notice_timer: crate::transient_notice::NoticeTimer<ClientNotice>,
    setup_notice_timer: crate::transient_notice::NoticeTimer<SetupStatus>,
    focus_new_panel: bool,
    settings: settings::Coordinator,
    launch_profiles: launch_profiles::Coordinator,
    preferences: orchbee_workbench_protocol::UserPreferencesValues,
    completion_reminders: super::completion_reminders::State,
    notification_base: (u64, orchbee_local_runtime::NotificationSettings),
    menus: action_menus::Coordinator,
    mailbox: Option<Arc<CommandMailbox>>,
    clock: LocalOperationClock,
    outer_focused: Option<bool>,
    state: Option<WorkbenchClientState>,
    board: super::agent_board::AgentBoard,
    worktrees: worktree_launch::Coordinator,
    attention: super::agent_attention::AgentAttention,
    reports: super::panel_reports::PanelReports,
    graphics: super::pane_graphics::Driver,
    overlays: super::plugin_overlays::Driver,
    links: crate::terminal_link_activation::Driver,
    copy_mode: copy_mode::Coordinator,
    popup: super::independent_popup::Driver,
    command_notice: Option<(WorkbenchClientNotice, std::time::Instant)>,
    notice_context: Option<NoticeContext>,
    /// `presentation_id → workbench placement` over the freshest observed
    /// canonical snapshot — a pure function of that snapshot, rebuilt
    /// whole on every snapshot-bearing event.
    agent_index: super::agent_board::PresentationIndex,
    projections: Vec<KeyedProjection>,
    retained_frames: retained_frames::RetainedFrames,
    navigation_draw: navigation_draw::NavigationDraw,
    history_positions: history_positions::Coordinator,
    effects: super::terminal_effects::Queue,
    terminal_effect: Option<super::terminal_effects::Job>,
    clipboard: super::terminal_clipboard::Writer,
    pending_draws: Vec<PendingDraw>,
    poll_cursor: usize,
    ordinary_poll_serial: Option<u64>,
    next_empty_poll: Option<std::time::Instant>,
    unbound_refresh_at: Option<std::time::Instant>,
    retirement_failure: Option<CarrierFailure>,
    retirement_finalized: bool,
    retirement: Option<DisplayRetirement>,
    retired_mailbox_seed: Option<super::mailbox::RetiredMailboxSeed>,
    retirement_poll_serial: Option<u64>,
    retired_peer: Option<super::state::retired_membership::RetiredMembershipOrigin>,
    retired_peer_attempt: Option<retired_membership::RetiredMembershipAttempt>,
}

mod scrollbar;

mod client_presentation;
mod client_viewport;
mod completion_reminders;
mod independent_popup;
mod input_capture;
mod link_activation;
mod navigation_draw;
mod terminal_tail;

impl WorkbenchClientUi {
    pub(super) fn set_ui_wakeup(&self, wakeup: Option<super::UiWakeup>) {
        if let Some(mailbox) = &self.mailbox {
            mailbox.set_ui_wakeup(wakeup);
        }
    }

    pub(super) fn next_poll_delay(&self) -> Option<std::time::Duration> {
        if self.retirement_failure.is_some() {
            return None;
        }
        let ordinary = self
            .next_empty_poll
            .filter(|_| self.ordinary_poll_serial.is_none());
        // The sequence owns admission while its lease settles; an expired ordinary poll
        // must not busy-spin. Keep independent close and summary deadlines below.
        let ordinary = ordinary.filter(|_| {
            !self
                .state
                .as_ref()
                .is_some_and(WorkbenchClientState::duplicate_sequence_active)
        });
        let close = self
            .state
            .as_ref()
            .and_then(WorkbenchClientState::close_set_deadline);
        let summary = self
            .state
            .as_ref()
            .and_then(WorkbenchClientState::close_set_summary_deadline);
        ordinary
            .into_iter()
            .chain(close)
            .chain(summary)
            .min()
            .map(|at| at.saturating_duration_since(self.clock.now()))
    }

    pub(super) fn host_focus(&self) -> Option<bool> {
        self.outer_focused
    }
    pub(super) fn outer_title_focused(&self) -> bool {
        self.outer_focused.unwrap_or(false)
    }
    pub(super) fn configuration_clock(&self) -> LocalOperationClock {
        self.clock.clone()
    }
    pub(super) fn configure_panel_focus(&mut self, focus_new_panel: bool) {
        self.focus_new_panel = focus_new_panel;
    }

    #[cfg(test)]
    pub(super) fn new(mailbox: Arc<CommandMailbox>, clock: LocalOperationClock) -> Self {
        Self::with_capabilities(mailbox, clock, crate::tui_capabilities::Capabilities::all())
    }
    pub(super) fn with_capabilities(
        mailbox: Arc<CommandMailbox>,
        clock: LocalOperationClock,
        capabilities: crate::tui_capabilities::Capabilities,
    ) -> Self {
        Self {
            capabilities,
            mailbox: Some(mailbox),
            board: super::agent_board::AgentBoard::new(
                clock.clone(),
                capabilities.supports(crate::tui_capabilities::Capability::AgentLaunch),
            ),
            worktrees: worktree_launch::Coordinator::default(),
            menus: action_menus::Coordinator::default(),
            settings: settings::Coordinator::new(capabilities.login_shell),
            notice_timer: Default::default(),
            setup_notice_timer: Default::default(),
            focus_new_panel: true,
            launch_profiles: launch_profiles::Coordinator::default(),
            preferences: orchbee_workbench_protocol::UserPreferencesValues::default(),
            completion_reminders: super::completion_reminders::State::default(),
            notification_base: (0, orchbee_local_runtime::NotificationSettings::default()),
            attention: super::agent_attention::AgentAttention::new(clock.clone()),
            reports: super::panel_reports::PanelReports::new(clock.clone()),
            graphics: super::pane_graphics::Driver::new(clock.clone()),
            overlays: super::plugin_overlays::Driver::new(clock.clone()),
            links: crate::terminal_link_activation::Driver::new(clock.clone()),
            copy_mode: copy_mode::Coordinator::new(clock.clone()),
            popup: super::independent_popup::Driver::new(clock.clone()),
            command_notice: None,
            notice_context: None,
            agent_index: super::agent_board::PresentationIndex::default(),
            clock,
            state: None,
            outer_focused: None,
            projections: Vec::new(),
            retained_frames: retained_frames::RetainedFrames::default(),
            navigation_draw: navigation_draw::NavigationDraw::default(),
            history_positions: history_positions::Coordinator::default(),
            effects: super::terminal_effects::Queue::default(),
            terminal_effect: None,
            clipboard: super::terminal_clipboard::Writer::default(),

            pending_draws: Vec::new(),
            poll_cursor: 0,
            ordinary_poll_serial: None,
            next_empty_poll: None,
            unbound_refresh_at: None,
            retirement_failure: None,
            retirement_finalized: false,
            retirement: None,
            retired_mailbox_seed: None,
            retirement_poll_serial: None,
            retired_peer: None,
            retired_peer_attempt: None,
        }
    }

    pub(super) fn view(&self) -> WorkbenchClientView<'_> {
        let command_notice = self
            .settings
            .notice(self.clock.now())
            .map(WorkbenchClientNotice::Settings);
        let command_notice = command_notice
            .or_else(|| self.links.notice().map(link_notice))
            .or_else(|| {
                self.command_notice
                    .filter(|(_, until)| self.clock.now() < *until)
                    .map(|(notice, _)| notice)
            });
        let state = self.state.as_ref();
        let focus = state.and_then(WorkbenchClientState::focus_panel_id);
        // Drop projections whose exact incarnation no longer exists — the
        // settle paths normally remove them, this filter is the last fence.
        let focus_status = focus
            .and_then(|panel| {
                state
                    .and_then(|state| state.pane_by_panel(panel))
                    .map(|record| record.pane_ref())
            })
            .and_then(|key| {
                self.projections
                    .iter()
                    .find(|projection| projection.key == key)
            })
            .map_or(ClientProjectionStatus::Empty, |projection| {
                projection.status
            });
        WorkbenchClientView {
            resize: None,
            state: state.map(WorkbenchClientState::view),
            panes: &self.projections,
            retained_frames: state.map(|state| self.retained_frames.view(state)),
            focus,
            projection_status: focus_status,
            retirement: self.retirement,
            retirement_requested: self.retirement_failure.is_some(),
            board: self.agent_board_view(),
            board_summary: self.board.summary(),
            copy_mode: Some(&self.copy_mode),
            attention: Some(&self.attention),
            reports: state
                .filter(|s| self.reports.matches(s))
                .map(|_| &self.reports),
            graphics: state
                .filter(|_| self.retirement.is_none())
                .map(|_| &self.graphics),
            graphics_surface: None,
            popup_failed: self.popup.failed(),
            command_notice,
            notice_context: self.notice_context.as_ref(),
            popup: Some(&self.popup),
            worktree: Some(&self.worktrees.launcher),
            menu: Some(&self.menus),
            settings: Some(&self.settings),
            completion_reminder: self
                .retirement
                .is_none()
                .then(|| self.completion_reminders.reminder(self.clock.now()))
                .flatten(),
        }
    }

    pub(super) fn retained_frame_bank(&self) -> RetainedFrameBank {
        RetainedFrameBank {
            frames: self.retained_frames.clone(),
            history: self.history_positions.bank.clone(),
        }
    }

    pub(super) fn inherit_retained_frame_bank(&mut self, older: RetainedFrameBank) {
        if let Some(state) = &self.state {
            self.retained_frames.inherit_older(older.frames, state);
            {
                self.history_positions
                    .bank
                    .inherit_older(older.history, state);
                self.history_positions.projections_cleared();
            }
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
                Some(DisplayRetirement::OutcomeUnknown) => TerminalStatus::OutcomeUnknown,
                Some(DisplayRetirement::Unavailable) => TerminalStatus::Unavailable,
                None => TerminalStatus::Connecting,
            };
        };
        if self.view().projection_status() == ClientProjectionStatus::Blocked {
            return TerminalStatus::Blocked;
        }
        match (state.phase_kind(), state.controller_mode()) {
            (
                ClientPhaseKind::Ready
                | ClientPhaseKind::MoveProjectionUnavailable
                | ClientPhaseKind::RenameProjectionUnavailable,
                Some(ControllerMode::Controlling(_)),
            ) => TerminalStatus::Controlling,
            (
                ClientPhaseKind::Ready
                | ClientPhaseKind::MoveProjectionUnavailable
                | ClientPhaseKind::RenameProjectionUnavailable,
                Some(ControllerMode::ReadOnly),
            ) => TerminalStatus::ReadOnly,
            (ClientPhaseKind::Retired, _) => match self.retirement {
                Some(DisplayRetirement::OutcomeUnknown) => TerminalStatus::OutcomeUnknown,
                Some(DisplayRetirement::Unavailable) | None => TerminalStatus::Unavailable,
            },
            (
                ClientPhaseKind::SelectedWithoutAttachment
                | ClientPhaseKind::RetryableAttachUnavailable
                | ClientPhaseKind::ProjectionUnavailable,
                _,
            ) => TerminalStatus::Unavailable,
            _ => TerminalStatus::Connecting,
        }
    }

    pub(super) fn startup_draw_ready(&self) -> bool {
        self.retirement_failure.is_none()
            && (!self.pending_draws.is_empty()
                || self.state.as_ref().is_some_and(|state| {
                    matches!(
                        state.phase_kind(),
                        ClientPhaseKind::Empty
                            | ClientPhaseKind::SelectedWithoutAttachment
                            | ClientPhaseKind::RetryableAttachUnavailable
                    )
                }))
    }

    pub(super) const fn retirement_requested(&self) -> bool {
        self.retirement_failure.is_some() && !self.retirement_finalized
    }

    pub(super) fn release_mailbox(&mut self) {
        if let Some(mailbox) = &self.mailbox {
            self.retired_mailbox_seed = Some(mailbox.retired_seed());
        }
        self.mailbox.take();
    }

    #[cfg(test)]
    pub(super) fn committed_target_retained_for_test(&self) -> bool {
        self.state
            .as_ref()
            .and_then(WorkbenchClientState::committed_target)
            .is_some()
    }

    #[cfg(test)]
    pub(super) fn move_retirement_for_test(&self) -> Option<(&PendingMoveCommit, CarrierFailure)> {
        let state = self.state.as_ref()?;
        Some((state.retained_move_for_test()?, state.retired_failure()?))
    }

    #[cfg(test)]
    pub(super) fn latest_geometry_for_test(&self) -> Option<Geometry> {
        self.state
            .as_ref()
            .map(WorkbenchClientState::latest_geometry)
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
            .and_then(WorkbenchClientState::retired_failure)
            .unwrap_or(failure);
        self.retirement_failure = Some(failure);
        self.retained_frames.clear();
        {
            self.history_positions = history_positions::Coordinator::default();
        }
        self.clear_projections();
        self.retirement_finalized = true;
        self.retirement = Some(if failure == CarrierFailure::OutcomeUnknown {
            DisplayRetirement::OutcomeUnknown
        } else {
            DisplayRetirement::Unavailable
        });
        accepted
    }

    #[cfg(test)]
    pub(super) fn tick(&mut self) -> bool {
        self.tick_with_client_capture(crate::tui_composition::ClientCaptureClaims::default())
    }

    /// Advances native domain work while honoring the same client-over-native
    /// capture projection used by paint and input dispatch.
    pub(super) fn tick_with_client_capture(
        &mut self,
        client_capture: crate::tui_composition::ClientCaptureClaims,
    ) -> bool {
        let duplicate_changed = self
            .state
            .as_mut()
            .is_some_and(|state| state.observe_duplicate_sequence(self.clock.now()));

        if let Some(state) = &self.state {
            self.retained_frames.prune(state);
        }
        let command_notice_changed = if self
            .command_notice
            .is_some_and(|(_, until)| self.clock.now() >= until)
        {
            self.command_notice = None;
            true
        } else {
            false
        };

        let effect_changed = self.settle_effect_delivery();
        if self.retirement_failure.is_some() {
            return command_notice_changed || duplicate_changed;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return command_notice_changed;
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
            Err(EventError::ForwardExpired) => {
                return command_notice_changed || acknowledged || !self.pending_draws.is_empty();
            }
            Err(EventError::Closed(closed)) => {
                self.retire(closed.failure());
                return true;
            }
        };
        let mut changed = match taken {
            Some(taken) => self.apply_event(&mailbox, taken),
            None => false,
        };
        changed |= duplicate_changed;
        // An armed membership-recovery attempt owns the mailbox until its
        // candidate installs or the attempt settles: suppress the ordinary
        // post-event scheduler block so the queue carries the Refresh alone.
        if self.retirement_failure.is_none() && !self.retired_membership_attempt_active() {
            changed |= self
                .state
                .as_mut()
                .is_some_and(|state| state.settle_stale_close_confirmations());
            self.reconcile_ordinary_poll(&mailbox);
            {
                changed |= self.drive_duplicate_sequence(&mailbox);
            }
            changed |= self.state.as_mut().is_some_and(|state| {
                state.settle_stale_workspace_close()
                    | state.settle_stale_quick_workspace()
                    | state.settle_stale_project_rename()
                    | state.tick_workspace_close_pending(self.clock.now())
                    | state.tick_takeover(self.clock.now())
                    | state.settle_close_set(self.clock.now())
            });
            self.drain_publication_refresh(&mailbox);
            self.schedule_close_reconciliation(&mailbox);
            self.schedule_close_read(&mailbox);
            changed |= self.schedule_close_set(&mailbox);
            self.schedule_pane_reconcile(&mailbox);
            self.schedule_poll(&mailbox);
            changed |= self.schedule_unbound_refresh(&mailbox);
            // Board lane settlement and intent scheduling remain fair while a
            // client surface covers native content. Only a focus-moving jump
            // must wait until the total compositor names Board as the top.
            let allow_agent_board_jump =
                crate::tui_composition::project_capture(client_capture, self.capture_claims())
                    == Some(crate::tui_composition::CaptureOwner::Native(
                        crate::tui_composition::WorkbenchCaptureOwner::AgentBoard,
                    ));
            changed |= self.drive_agent_board(allow_agent_board_jump);
            {
                if let Some(state) = self.state.as_mut()
                    && self.overlays.tick(state)
                {
                    state.note_publication_hint();
                    changed = true;
                }
                changed |= self.drive_worktree();
                changed |= self.drive_action_menu();
                changed |= self.links.tick();
                changed |= self.popup.tick();
                changed |= self.graphics.tick(self.state.as_ref());
                changed |= self.reports.tick(
                    self.state
                        .as_ref()
                        .and_then(super::panel_reports::Scope::capture),
                );
            }
            // Viewport deadline episodes and staged steps advance on the
            // same fair tick as the ordinary schedulers.
            changed |= self.drive_settings();
            changed |= self.attention.tick();
            changed |= self.drive_completion_reminders();
            {
                changed |= self.drive_history_positions();
            }
            changed |= self.drive_terminal_tails(&mailbox);
            changed |= self.tick_viewports(&mailbox);
            self.admit_selections(&mailbox);
            {
                changed |= self.tick_copy_mode();
            }
        }
        changed |= self.expire_status_notices();
        acknowledged
            || command_notice_changed
            || changed
            || effect_changed
            || self.effects.has_pending()
            || !self.pending_draws.is_empty()
    }

    pub(super) fn after_draw(&mut self) {
        // The shell calls this only after Backend::flush closed the frame.
        self.remember_navigation_draw();
        self.remember_history_positions();
        if let Some(state) = &self.state {
            self.retained_frames.after_draw(state, &self.projections);
        }
        self.dispatch_effect_after_draw();
        self.settle_pending_polls(true);
    }

    fn settle_pending_polls(&mut self, presented: bool) {
        if self.pending_draws.is_empty() {
            return;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return;
        };
        // Each pane's poll keeps its own causal slot. A held navigation frame
        // settles only non-frame events; live pixels still require a completed draw.
        let pending_draws = std::mem::take(&mut self.pending_draws);
        for pending in pending_draws {
            if !presented
                && pending.frame.is_some()
                && self
                    .state
                    .as_ref()
                    .is_some_and(|state| state.pane_projected(pending.poll_key.pane_ref()))
            {
                self.pending_draws.push(pending);
                continue;
            }
            // The settlement claims this draw's proof: a frame that was never
            // staged into a live, non-departing leaf settles as `NoFeedback` —
            // the poll-time `FrameApplied` is never forwarded for a frame the
            // screen never showed.
            let settlement = match pending.frame {
                Some(frame) => {
                    let staged = presented
                        && self.state.as_mut().is_some_and(|state| {
                            state.stage_drawn_frame(pending.poll_key.pane_ref(), frame)
                        });
                    if staged {
                        pending.settlement
                    } else {
                        CausalSettlement::NoFeedback
                    }
                }
                None => pending.settlement,
            };
            if pending.ordinary && pending.frame.is_none() {
                self.ordinary_poll_serial = None;
            }
            if let Err(closed) = mailbox.deposit_causal(&pending.poll_key, settlement) {
                self.retire(closed.failure());
                return;
            }
        }
    }

    pub(super) fn observe_focus(&mut self, focused: bool) {
        self.inherit_host_focus(Some(focused));
    }
    pub(super) fn inherit_host_focus(&mut self, focused: Option<bool>) {
        if self.outer_focused != focused {
            if let Some(state) = &mut self.state {
                state.cancel_duplicate_sequence();
            }
            self.outer_focused = focused;
            self.attention.focus_changed();
        }
    }

    fn observe_focus_geometry(&mut self, geometry: Geometry) {
        self.observe_action_menu_geometry(geometry);
        if self
            .state
            .as_ref()
            .is_some_and(|state| state.latest_geometry() != geometry)
        {
            self.invalidate_close_display();
            self.invalidate_rename_display();
            self.invalidate_setup_display();
            {
                self.after_worktree_draw(None);
            }
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
        }
    }

    pub(super) fn expedite_agent_reads(&mut self) {
        self.attention.expedite();
        self.reports.expedite();
    }

    /// Publishes the outer terminal's settled default colors for Host-default terminals.
    pub(super) fn observe_host_colors(&mut self, colors: Option<(u32, u32)>) {
        if self.retirement_failure.is_some() {
            return;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return;
        };
        if let Err(closed) = mailbox.observe_host_colors(colors) {
            self.retire(closed.failure());
        }
    }

    pub(super) fn observe_geometry(&mut self, geometry: Geometry) {
        self.observe_focus_geometry(geometry);
        // A whole-session observation on a single-leaf tab is exactly that
        // leaf's geometry — route it through the pane path so the focus
        // viewer's resize is emitted; multi-leaf tabs own per-leaf geometry
        // through `observe_panes`.
        let single_leaf = self.state.as_ref().and_then(|state| {
            let active = state.active()?;
            state
                .cache()
                .tabs()
                .get(active.index())
                .filter(|tab| tab.panel_count() == 1)?;
            Some((state.focus_panel_id()?.clone(), active.panel_index()))
        });
        if let Some((panel_id, panel_index)) = single_leaf {
            self.observe_panes(&[(panel_id, panel_index, geometry)]);
        }
    }

    /// The leaf-geometry report the shell paints with: each `(panel_id,
    /// panel_index, content geometry)` entry syncs one pane's record. The
    /// report installs/departs records, resizes visible controlling panes when a leaf
    /// moved, and schedules the reconcile snapshot that converges viewers.
    pub(super) fn observe_panes(&mut self, observed: &[(WorkbenchPanelId, usize, Geometry)]) {
        if self.retirement_failure.is_some() {
            return;
        }
        let focus_geometry = self.state.as_ref().and_then(|state| {
            let focus = state.focus_panel_id()?;
            observed
                .iter()
                .find(|(panel, _, _)| panel == focus)
                .map(|(_, _, geometry)| *geometry)
        });
        if let Some(geometry) = focus_geometry {
            self.observe_focus_geometry(geometry);
            if self.retirement_failure.is_some() {
                return;
            }
        }
        // An armed membership-recovery attempt owns the mailbox: pane records
        // are epoch-owned canonical facts, so the observation's record sync,
        // Resize admission, and reconcile enqueue all stay outside the window.
        // The recovery mailbox carries the Refresh alone and the install-time
        // origin check still sees the untouched canonical pane set.
        if self.retired_membership_attempt_active() {
            return;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return;
        };
        let sampled_at = self.clock.now();
        let resizes = {
            let Some(state) = self.state.as_mut() else {
                return;
            };
            // A non-Client close carrier — status-only recovery, or the
            // pending membership handoff before its atomic install — owns the
            // window the same way: the carrier mailbox holds no canonical pane
            // set, so the observation's record sync would mint pane facts the
            // Retired-phase shape forbids, and Resize admission has no
            // attached record to carry it. Legitimate close scheduling is
            // unaffected; the predicate releases with the carrier's own
            // settlement commit.
            if state.close_status_only() {
                return;
            }
            state.sync_pane_set(observed, sampled_at);
            // First-Starting records the observation but admits no ordinary
            // Resize: the focus leaf's setup pipeline owns its geometry
            // convergence, and the ordinary queue must stay empty for the
            // startup allocator. Post-Ready observations admit the latest
            // recorded geometry normally.
            if state.phase_kind() == ClientPhaseKind::Starting {
                Vec::new()
            } else {
                state.admit_observed_geometries(observed)
            }
        };
        for (key, geometry) in resizes {
            let Some(state) = self.state.as_mut() else {
                return;
            };
            if state.close_suppresses_attachment(key.pane().anchor()) {
                continue;
            }
            match mailbox.try_enqueue(QueuedCommand::Resize(Tagged::new(key.clone(), geometry))) {
                Ok(_serial) => {
                    if !state.record_admitted_geometry(&key, geometry) {
                        self.retire(CarrierFailure::Protocol);
                        return;
                    }
                    // The host applies the emitted geometry to this
                    // incarnation: a non-Live core's window can never paint
                    // under it again, so its geometry tracks the request.
                    if let Some(viewport) = self
                        .projections
                        .iter_mut()
                        .find(|projection| projection.key == key)
                        .and_then(|projection| projection.viewport.as_mut())
                    {
                        let effect =
                            viewport.resize(geometry.columns(), geometry.rows(), sampled_at);
                        self.settle_viewport_effect(&mailbox, &key, effect);
                        if self.retirement_failure.is_some() {
                            return;
                        }
                    }
                }
                Err(AdmissionError::Full | AdmissionError::Busy) => {}
                Err(AdmissionError::Closed(closed)) => {
                    self.retire(closed.failure());
                    return;
                }
                Err(
                    AdmissionError::BootstrapPending
                    | AdmissionError::StaleAttachment
                    | AdmissionError::AttachmentGenerationExhausted
                    | AdmissionError::RefreshGenerationExhausted
                    | AdmissionError::Build(_),
                ) => {
                    self.retire(CarrierFailure::Protocol);
                    return;
                }
            }
        }
        self.schedule_pane_reconcile(&mailbox);
    }

    /// Enqueues the reconcile snapshot the state computed when its desired
    /// pane set diverged from live attachments. Dedup is the mailbox's.
    /// The first Starting phase defers this ordinary convergence work: the
    /// divergence `sync_pane_set` recorded (and its minted episode budget)
    /// stays visible through `reconcile_pending` while the queue the startup
    /// allocator requires empty carries nothing. Ready's first schedule
    /// delivers the same `PaneRef` targets under that original deadline. An
    /// armed membership-recovery attempt suppresses the enqueue from every
    /// caller: its mailbox carries the recovery Refresh alone. A non-Client
    /// close carrier suppresses it the same way until the carrier's own
    /// settlement commits: the status mailbox's desired anchor is not the
    /// session's retained anchor, so an ordinary admission would mint work it
    /// must reject as stale.
    fn schedule_pane_reconcile(&mut self, mailbox: &Arc<CommandMailbox>) {
        if self.retired_membership_attempt_active() {
            return;
        }
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if state.phase_kind() == ClientPhaseKind::Starting || state.close_status_only() {
            return;
        }
        let Some((anchor, targets, deadlines)) = state.reconcile_targets(self.clock.now()) else {
            return;
        };
        match mailbox.try_enqueue_reconcile(anchor, targets, deadlines) {
            Ok(_) | Err(AdmissionError::Full | AdmissionError::Busy) => {}
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

    /// Whether an armed retired-membership recovery attempt currently owns the
    /// mailbox: ordinary scheduled admissions stay suppressed until the
    /// candidate installs or the attempt settles.
    fn retired_membership_attempt_active(&self) -> bool {
        self.retired_peer_attempt.is_some()
    }

    pub(super) fn forward(&mut self, input: &[u8]) -> bool {
        if self.retirement_failure.is_some() {
            return false;
        }
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        if self.workspace_creation_replaces_content() {
            return false;
        }
        if !state.takeover_input_ready() {
            return false;
        }
        let Some(ready) = state.ready() else {
            return false;
        };
        if !matches!(ready.mode(), ControllerMode::Controlling(_)) {
            return false;
        }
        let Some(desired) = state.focus_pane_ref() else {
            return false;
        };
        if state.close_suppresses_attachment(desired.pane().anchor()) {
            return false;
        }
        let Ok(input) = ControlInput::new(input.to_vec()) else {
            return false;
        };
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return false;
        };
        // A non-Live focus pane never receives raw bytes: Entering/Viewing
        // stage `Recover` first and the byte follows only once that step is
        // admitted onto the wire queue; Recovering already holds its
        // admission and forwards immediately. A byte that cannot get behind
        // an admitted Recover is dropped — never replayed, never forwarded
        // while the pane still scrolls.
        let recovering = self
            .projections
            .iter()
            .find(|projection| projection.key == desired)
            .and_then(|projection| projection.viewport.as_ref())
            .is_some_and(TerminalViewport::recovering);
        if !recovering
            && let Some(viewport) = self
                .projections
                .iter_mut()
                .find(|projection| projection.key == desired && projection.tail.is_none())
                .and_then(|projection| projection.viewport.as_mut())
        {
            viewport.typed();
            self.admit_viewport_steps(&mailbox, &desired);
            let admitted = self
                .projections
                .iter()
                .find(|projection| projection.key == desired)
                .and_then(|projection| projection.viewport.as_ref())
                .is_some_and(TerminalViewport::recovering);
            if !admitted {
                return false;
            }
        }
        match mailbox.try_enqueue(QueuedCommand::Input(Tagged::new(desired, input))) {
            Ok(_serial) => {
                self.next_empty_poll = None;
                true
            }
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

    /// Enters retained-history mode on the exact focus incarnation: the
    /// `Enter` step must be admitted before the deterministic core installs
    /// — a refused enqueue leaves the pane Live, and no input is ever
    /// replayed on its behalf.
    fn enter_history(&mut self) -> ClientActionDisposition {
        self.enter_history_rows(None)
    }

    /// Keyboard entry uses a page; pointer entry supplies its line count.
    /// Both share the same admission, incarnation and demand ownership.
    fn enter_history_rows(&mut self, rows_before: Option<u32>) -> ClientActionDisposition {
        let Some(key) = self.state.as_ref().and_then(|state| state.focus_pane_ref()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        self.enter_pane_history_rows(&key, rows_before)
    }

    /// Enter the exact readable pane without changing keyboard focus or control.
    fn enter_pane_history_rows(
        &mut self,
        key: &PaneRef,
        rows_before: Option<u32>,
    ) -> ClientActionDisposition {
        if self.workspace_creation_replaces_content() {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let Some(state) = self.state.as_ref() else {
            return ClientActionDisposition::KeepNavigator;
        };
        if state.ready().is_none() {
            return ClientActionDisposition::KeepNavigator;
        }
        if !state.pane_projected(key) {
            return ClientActionDisposition::KeepNavigator;
        }
        if state.close_suppresses_attachment(key.pane().anchor()) {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(index) = self.projections.iter().position(|p| &p.key == key) else {
            return ClientActionDisposition::KeepNavigator;
        };
        if self.viewport_admission_blocked(key) {
            return ClientActionDisposition::KeepNavigator;
        }
        if self.projections[index].viewport.is_some() {
            if self.projections[index].tail.take().is_some() {
                let rows = rows_before.unwrap_or_else(|| {
                    u32::from(
                        self.projections[index]
                            .viewport
                            .as_ref()
                            .expect("existing viewport")
                            .geometry()
                            .1,
                    )
                });
                let effect = self.projections[index]
                    .viewport
                    .as_mut()
                    .expect("existing viewport")
                    .nav(ViewportNav::Lines(i64::from(rows)));
                self.settle_viewport_effect(&mailbox, key, effect);
            }
            return ClientActionDisposition::KeepNavigator;
        }
        // Readable retained rows or a cropped current screen need a live-tail
        // anchor. An alternate screen or a missing/empty frame has no ordinary
        // history contract. Refusal reports an honest notice and emits no Enter.
        let Some(frame) = self.projections[index].value.frame() else {
            return self.history_unavailable(&mailbox);
        };
        let clipped = self
            .view()
            .pane_live_view(key.pane().panel())
            .is_some_and(|view| view.clipped);
        if (frame.scrollback_rows == 0 && !clipped)
            || frame.alternate_screen
            || frame.rows.is_empty()
        {
            return self.history_unavailable(&mailbox);
        }
        let (columns, row_count) = (frame.geometry.columns, frame.geometry.rows);
        let first_read_sequence = self.projections[index].read_watermark.saturating_add(1);
        match mailbox.try_enqueue(QueuedCommand::Viewport(Tagged::new(
            key.clone(),
            ViewportStep::Enter,
        ))) {
            Ok(_) => {
                self.projections[index].viewport = Some(TerminalViewport::entering(
                    *self.projections[index].value.attachment(),
                    *self.projections[index].value.epoch(),
                    columns,
                    row_count,
                    rows_before.unwrap_or_else(|| u32::from(row_count)),
                    first_read_sequence,
                    self.clock.now(),
                ));
            }
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
        ClientActionDisposition::KeepNavigator
    }

    /// The Enter request cannot honestly proceed: publish
    /// `HistoryUnavailable` and send nothing on the wire.
    fn history_unavailable(&mut self, mailbox: &Arc<CommandMailbox>) -> ClientActionDisposition {
        if let Some(state) = self.state.as_mut() {
            mailbox.apply_local_for(state, |state| {
                state.set_notice(ClientNotice::HistoryUnavailable);
            });
        }
        ClientActionDisposition::KeepNavigator
    }

    /// Whether the focus pane's non-Live core consumes this key locally.
    /// The five history keys scroll or exit; every other key reports
    /// `false` so the ordinary byte path forwards (and exits history
    /// first). Live panes always report `false` — their passthrough is
    /// untouched.
    pub(super) fn history_key(&mut self, event: &crate::client_view::KeyEvent) -> bool {
        if self.workspace_creation_replaces_content() {
            return false;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return false;
        };
        let Some(key) = self.state.as_ref().and_then(|state| state.focus_pane_ref()) else {
            return false;
        };
        let Some(index) = self.projections.iter().position(|p| p.key == key) else {
            return false;
        };
        if self.projections[index].tail.is_some() {
            return false;
        }
        let Some(viewport) = self.projections[index].viewport.as_mut() else {
            return false;
        };
        let (_, row_count) = viewport.geometry();
        let nav = match event.normalized() {
            (crate::client_view::KeyCode::PageUp, mods)
                if mods == crate::client_view::Modifiers::NONE =>
            {
                ViewportNav::Lines(i64::from(row_count))
            }
            (crate::client_view::KeyCode::PageDown, mods)
                if mods == crate::client_view::Modifiers::NONE =>
            {
                ViewportNav::Lines(-i64::from(row_count))
            }
            (crate::client_view::KeyCode::Home, mods)
                if mods == crate::client_view::Modifiers::NONE =>
            {
                ViewportNav::Oldest
            }
            (crate::client_view::KeyCode::End, mods) | (crate::client_view::KeyCode::Esc, mods)
                if mods == crate::client_view::Modifiers::NONE =>
            {
                ViewportNav::Exit
            }
            _ => return false,
        };
        let effect = viewport.nav(nav);
        self.settle_viewport_effect(&mailbox, &key, effect);
        true
    }

    /// A wheel notch enters eligible Live history or scrolls its existing
    /// core by three rows. Recheck the readable incarnation at this mutation boundary;
    /// the pointer layer owns content hit testing and Shift exclusion.
    pub(super) fn history_wheel(&mut self, key: &PaneRef, up: bool) -> bool {
        if self.workspace_creation_replaces_content() {
            return false;
        }
        if self.retirement_failure.is_some()
            || self.state.as_ref().is_none_or(|state| {
                state.ready().is_none()
                    || !state.pane_projected(key)
                    || state.close_suppresses_attachment(key.pane().anchor())
            })
            || self.viewport_admission_blocked(key)
        {
            return false;
        }
        if self.copy_mode.active() {
            return false;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return false;
        };
        let Some(index) = self.projections.iter().position(|p| &p.key == key) else {
            return false;
        };
        if self.projections[index].viewport.is_none() {
            let eligible = up
                && self.projections[index].value.frame().is_some_and(|frame| {
                    !frame.alternate_screen
                        && frame.interaction.mouse_tracking()
                            == orchbee_terminal_protocol::MouseTracking::None
                        && (frame.scrollback_rows > 0
                            || self
                                .view()
                                .pane_live_view(key.pane().panel())
                                .is_some_and(|view| view.clipped))
                        && !frame.rows.is_empty()
                });
            if !eligible {
                return false;
            }
            self.enter_pane_history_rows(key, Some(3));
            return self
                .projections
                .iter()
                .any(|projection| &projection.key == key && projection.viewport.is_some());
        }
        let Some(viewport) = self.projections[index].viewport.as_mut() else {
            return false;
        };
        let effect = viewport.nav(ViewportNav::Lines(if up { 3 } else { -3 }));
        self.projections[index].tail = None;
        self.settle_viewport_effect(&mailbox, key, effect);
        true
    }

    /// Whether the exact focus incarnation currently carries a non-Live
    /// core — the pointer probe's fast fact for wheel ownership.
    fn focus_viewport_non_live(&self) -> bool {
        self.state
            .as_ref()
            .and_then(|state| state.focus_pane_ref())
            .is_some_and(|key| {
                self.projections.iter().any(|projection| {
                    projection.key == key
                        && projection.viewport.is_some()
                        && projection.tail.is_none()
                })
            })
    }

    /// Admits the viewport's next staged step onto the wire queue, in FIFO
    /// order. Busy/Full keeps the step staged for a later tick; a stale
    /// admission means the incarnation left the canonical set and its core
    /// is about to be disposed with the projection anyway, so the step
    /// simply stays pending.
    fn admit_viewport_steps(&mut self, mailbox: &Arc<CommandMailbox>, key: &PaneRef) {
        loop {
            if self.viewport_admission_blocked(key) {
                return;
            }
            let Some(projection) = self.projections.iter_mut().find(|p| &p.key == key) else {
                return;
            };
            let Some(viewport) = projection.viewport.as_mut() else {
                return;
            };
            let Some(step) = viewport.peek_command() else {
                return;
            };
            if let ViewportStep::Read {
                request_sequence, ..
            } = step
            {
                // The watermark advances when the owner observes the staged
                // read — admitted or not — so a sequence the core minted is
                // never reissued to this attachment's host watermark.
                projection.read_watermark = projection.read_watermark.max(request_sequence.get());
            }
            match mailbox.try_enqueue(QueuedCommand::Viewport(Tagged::new(key.clone(), step))) {
                Ok(_) => viewport.command_admitted(),
                Err(
                    AdmissionError::Full | AdmissionError::Busy | AdmissionError::StaleAttachment,
                ) => {
                    return;
                }
                Err(AdmissionError::Closed(closed)) => {
                    self.retire(closed.failure());
                    return;
                }
                Err(
                    AdmissionError::BootstrapPending
                    | AdmissionError::AttachmentGenerationExhausted
                    | AdmissionError::RefreshGenerationExhausted
                    | AdmissionError::Build(_),
                ) => {
                    self.retire(CarrierFailure::Protocol);
                    return;
                }
            }
        }
    }

    /// Applies one core-produced effect: a protocol fault retires the
    /// carrier, a notice surfaces through the session lane, `live` retires
    /// the core back to Live, and every surviving transition tries to admit
    /// its staged steps.
    fn settle_viewport_effect(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        key: &PaneRef,
        effect: ViewportEffect,
    ) {
        if effect.protocol_error {
            self.retire(CarrierFailure::Protocol);
            return;
        }
        if let Some(notice) = effect.notice
            && let Some(state) = self.state.as_mut()
        {
            mailbox.apply_local_for(state, |state| state.set_notice(notice));
        }
        if effect.live {
            if let Some(state) = self.state.as_mut()
                && state.focus_pane_ref().as_ref() == Some(key)
                && state.notice() == Some(ClientNotice::HistoryRecovering)
            {
                mailbox.apply_local_for(state, WorkbenchClientState::clear_notice);
            }
            if let Some(projection) = self.projections.iter_mut().find(|p| &p.key == key) {
                projection.viewport = None;
                projection.tail = None;
            }
            return;
        }
        self.admit_viewport_steps(mailbox, key);
    }

    /// One fire-and-forget worker outcome applies to the exact incarnation
    /// it names. The core re-validates request sequences and staleness
    /// itself; a pane whose core already retired drops the outcome
    /// entirely.
    fn apply_viewport_outcome(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        key: PaneRef,
        outcome: ViewportOutcome,
    ) -> bool {
        let now = self.clock.now();
        let Some(index) = self.projections.iter().position(|p| p.key == key) else {
            return false;
        };
        if self.projections[index].viewport.is_none() {
            return false;
        }
        let effect = {
            let viewport = self.projections[index]
                .viewport
                .as_mut()
                .expect("viewport presence checked above");
            match outcome {
                ViewportOutcome::Enter { applied } => viewport.enter_outcome(applied),
                ViewportOutcome::Window {
                    request_sequence,
                    window,
                } => {
                    if window.route.sequence != request_sequence {
                        self.retire(CarrierFailure::Protocol);
                        return true;
                    }
                    viewport.window(*window, now)
                }
                ViewportOutcome::ReadRejected {
                    request_sequence,
                    rejection,
                } => viewport.read_outcome(request_sequence, rejection, now),
                ViewportOutcome::Ack {
                    request_sequence,
                    accepted,
                } => viewport.ack_outcome(request_sequence, accepted),
                ViewportOutcome::Recovered => {
                    viewport.recovered();
                    ViewportEffect::default()
                }
            }
        };
        self.settle_viewport_effect(mailbox, &key, effect);
        true
    }

    /// Advances every live core's deadline episode and drains its staged
    /// steps — the per-tick driver that turns expired waits into recovery
    /// and idle intents into wire commands.
    fn tick_viewports(&mut self, mailbox: &Arc<CommandMailbox>) -> bool {
        let now = self.clock.now();
        let mut changed = false;
        for index in 0..self.projections.len() {
            if self.retirement_failure.is_some() {
                break;
            }
            let key = self.projections[index].key.clone();
            let Some(viewport) = self.projections[index].viewport.as_mut() else {
                continue;
            };
            let effect = viewport.tick(now);
            if effect != ViewportEffect::default() {
                changed = true;
                self.settle_viewport_effect(mailbox, &key, effect);
            } else {
                self.admit_viewport_steps(mailbox, &key);
            }
        }
        changed
    }

    /// The pointer-resolution facts the dispatcher probes per event:
    /// capture state, exact focus/desired pane identities, per-leaf
    /// incarnation facts, stable identities for hit-region resolution,
    /// and the focus pane's interaction contract. Everything is read
    /// fresh so pointer events never resolve against a snapshot taken at
    /// paint time.
    pub(super) fn pointer_probe(&mut self) -> crate::terminal_input::PointerProbe {
        use crate::terminal_input::{PointerFrameFacts, PointerPane, PointerProbe, PointerTab};
        // Draft probes take `&mut self`, so they run before the shared
        // view borrow.
        let input_captured = self.rename_input_context().is_some()
            || self.setup_input_context().is_some()
            || self.panel_close_confirmation_armed();
        let view = self.view();
        let mut probe = PointerProbe {
            input_captured: input_captured
                || (self.worktree_input_context().is_some() || self.action_menu_active())
                || view.close_confirmation().is_some()
                || view.workspace_close_confirmation().is_some(),
            ..PointerProbe::default()
        };
        if self.workspace_creation_replaces_content() {
            return probe;
        }
        let Some(state) = self.state.as_ref() else {
            return probe;
        };
        // `input_ready` mirrors `forward`'s own admission gate — Ready
        // phase, Controlling mode, an exact focus incarnation, and no
        // close suppression on its anchor. All are live state reads;
        // nothing is guessed from the pointer's own records.
        let focus_available = state
            .focus_pane_ref()
            .is_some_and(|focus| !state.close_suppresses_attachment(focus.pane().anchor()));
        probe.selection_ready =
            self.retirement_failure.is_none() && state.ready().is_some() && focus_available;
        probe.input_ready = self.retirement_failure.is_none()
            && state.takeover_input_ready()
            && state
                .ready()
                .is_some_and(|ready| matches!(ready.mode(), ControllerMode::Controlling(_)))
            && focus_available;
        probe.focus = state.focus_pane_ref();
        probe.desired = state.desired().cloned();
        probe.viewport_non_live = self.focus_viewport_non_live();
        probe.panes = state
            .pane_facts()
            .map(|(record, attached, mode)| PointerPane {
                right_click_reporting: view
                    .pane_projection(record.pane().panel())
                    .and_then(|projection| projection.frame())
                    .is_some_and(|frame| {
                        frame.interaction.mouse_tracking()
                            != orchbee_terminal_protocol::MouseTracking::None
                    }),
                right_click: (0..view.tab_count())
                    .filter_map(|index| view.tab(index))
                    .find(|tab| tab.tab_id() == record.anchor().tab_id())
                    .and_then(|tab| {
                        (0..tab.panel_count())
                            .filter_map(|index| tab.panel(index))
                            .find(|panel| panel.panel_id() == record.pane().panel())
                    })
                    .map_or(
                        orchbee_workbench_protocol::PanelRightClickTarget::Orchbee,
                        |panel| panel.right_click(),
                    ),
                frame_available: view.pane_projection_status(record.pane().panel())
                    == ClientProjectionStatus::FrameAvailable,
                record,
                attached,
                controlling: mode.map(|mode| matches!(mode, ControllerMode::Controlling(_))),
            })
            .collect();
        if let Some(tab) = state.active().and_then(|active| view.tab(active.index())) {
            probe.unbound = (0..tab.panel_count())
                .filter_map(|index| tab.panel(index))
                .filter_map(|panel| view.unbound_pane(panel))
                .collect();
        }
        probe.tabs = (0..view.tab_count())
            .filter_map(|index| view.tab(index))
            .map(|tab| PointerTab {
                id: tab.tab_id().clone(),
                position: tab.position(),
            })
            .collect();

        probe.frame = view
            .projection()
            .and_then(|projection| projection.frame())
            .map(|frame| PointerFrameFacts {
                tracking: frame.interaction.mouse_tracking(),
                format: frame.interaction.mouse_format(),
                cell_pixel: (
                    frame.geometry.cell_pixel_width,
                    frame.geometry.cell_pixel_height,
                ),
                alternate_screen: frame.alternate_screen,
                alternate_scroll: frame.interaction.alternate_scroll(),
                application_cursor: frame.interaction.application_cursor_keys(),
            });
        probe
    }

    fn admit_capability(&mut self, capability: crate::tui_capabilities::Capability) -> bool {
        if self.capabilities.supports(capability) {
            return true;
        }
        self.command_notice = self
            .clock
            .now()
            .checked_add(std::time::Duration::from_secs(5))
            .map(|until| (WorkbenchClientNotice::Unsupported(capability), until));
        false
    }

    pub(super) fn admit_client_action(&mut self, action: &ClientAction) -> bool {
        crate::tui_capabilities::Capability::for_action(action)
            .is_none_or(|capability| self.admit_capability(capability))
    }

    pub(super) fn apply_client_action(&mut self, action: ClientAction) -> ClientActionDisposition {
        if !self.admit_client_action(&action) {
            return ClientActionDisposition::KeepNavigator;
        }
        self.remember_history_positions();
        let resume_handoff = self.take_action_menu_resume_handoff(&action);
        if let Some(state) = self.state.as_mut() {
            // New intent wins even when its admission is refused.
            state.cancel_project_quick_entry();
        }
        self.cancel_action_menu();
        if self.retirement_failure.is_some() {
            return ClientActionDisposition::KeepNavigator;
        }
        if let Some(state) = self.state.as_mut() {
            state.dismiss_close_set_summary();
            state.clear_settled_setup_status();
        }
        // The board and every other surface are mutually exclusive: any
        // action that is not the board's own toggle closes it first —
        // navigator, help, sidebar, modals, and pane operations all take
        // the overlay slot back.
        if !matches!(action, ClientAction::ToggleAgentBoard) {
            self.board.close();
        }
        if matches!(
            action,
            ClientAction::RequestWorkspaceRename
                | ClientAction::RequestWorkspaceMove
                | ClientAction::RequestWorkspaceDirectory
                | ClientAction::RequestWorkspaceClose
                | ClientAction::RequestProjectRename
        ) && let Some(state) = self.state.as_mut()
            && matches!(
                state.cache().location(),
                super::ClientContextLocation::NoWorkspace
            )
        {
            state.set_notice(ClientNotice::NoWorkspace);
            return ClientActionDisposition::KeepNavigator;
        }
        match action {
            ClientAction::OpenActionMenu => ClientActionDisposition::KeepNavigator,
            ClientAction::OpenSettings => self.request_settings(),
            ClientAction::OpenMachines
            | ClientAction::ConfiguredCommand(_)
            | ClientAction::PreviousAgent
            | ClientAction::NextAgent
            | ClientAction::FocusAgent(_) => ClientActionDisposition::KeepNavigator,
            ClientAction::RequestWorktreeCreate => self.request_worktree(WorktreeMode::Create),
            ClientAction::RequestWorktreeOpen => self.request_worktree(WorktreeMode::Open),
            ClientAction::RequestQuickWorkspace | ClientAction::RequestWorkspaceCreate => {
                if self.request_quick_workspace() {
                    ClientActionDisposition::LayoutChanged
                } else {
                    ClientActionDisposition::KeepNavigator
                }
            }
            ClientAction::RequestWorkspaceAtDirectory(request) => {
                if self.request_quick_workspace_with_review(request.map(|request| *request), true) {
                    ClientActionDisposition::LayoutChanged
                } else {
                    ClientActionDisposition::KeepNavigator
                }
            }
            ClientAction::RequestProjectQuickWorkspace(request) => {
                if self.request_quick_workspace_for(Some(*request)) {
                    return ClientActionDisposition::LayoutChanged;
                } else if let Some(state) = self.state.as_mut() {
                    state.set_notice(super::state::ClientNotice::DraftCancelled);
                }
                ClientActionDisposition::KeepNavigator
            }
            ClientAction::RequestTargetedProjectRename(request) => {
                if self.request_project_rename(request).is_none()
                    && let Some(state) = self.state.as_mut()
                {
                    state.set_notice(super::state::ClientNotice::DraftCancelled);
                }
                ClientActionDisposition::KeepNavigator
            }
            ClientAction::RequestWorkspaceOrganization(request) => {
                if !self.request_workspace_organization(request)
                    && let Some(state) = self.state.as_mut()
                {
                    state.set_notice(super::state::ClientNotice::DraftCancelled);
                }
                ClientActionDisposition::KeepNavigator
            }
            ClientAction::RequestWorkspaceMove => {
                let request = self.state.as_ref().and_then(|state| {
                    let deadline = self
                        .clock
                        .now()
                        .checked_add(std::time::Duration::from_secs(30))?;
                    let super::ClientContextLocation::Workspace { workspace_id, .. } =
                        state.cache().location()
                    else {
                        return None;
                    };
                    super::WorkspaceOrganizationRequest::workspace(state, workspace_id, deadline)
                });
                if let Some(request) = request {
                    self.request_workspace_organization(Box::new(request));
                }
                ClientActionDisposition::KeepNavigator
            }
            ClientAction::RequestProjectCloseAll(project) => {
                let project =
                    project.or_else(|| self.state.as_ref()?.cache().project_id().cloned());
                project
                    .as_ref()
                    .map_or(ClientActionDisposition::KeepNavigator, |project| {
                        self.request_project_close_all(project)
                    })
            }
            ClientAction::RequestWorkspaceDirectory
            | ClientAction::RequestWorkspaceRename
            | ClientAction::RequestProjectRename => {
                use super::state::workspace_setup::SetupIntent;
                let intent = match action {
                    ClientAction::RequestWorkspaceDirectory => SetupIntent::ConfigureDirectory,
                    ClientAction::RequestWorkspaceRename => {
                        SetupIntent::RenameLocation(LocationLabelTarget::Workspace)
                    }
                    _ => SetupIntent::RenameLocation(LocationLabelTarget::Project),
                };
                self.request_setup_draft(intent);
                ClientActionDisposition::KeepNavigator
            }
            ClientAction::TabMenu { action, tab_id } => match (action, resume_handoff) {
                (
                    crate::action_menu::MenuAction::ResumeTabClose,
                    action_menus::TabCloseResumeHandoff::Menu(permit),
                ) => tab_id
                    .as_ref()
                    .map_or(ClientActionDisposition::KeepNavigator, |tab_id| {
                        self.request_tab_close_resume_with_permit(tab_id, &permit)
                    }),
                (
                    crate::action_menu::MenuAction::ResumeTabClose,
                    action_menus::TabCloseResumeHandoff::Refused,
                ) => ClientActionDisposition::KeepNavigator,
                (action, _) => self.apply_tab_menu(action, tab_id.as_ref()),
            },
            ClientAction::ToggleTabGroup(group) => self.toggle_tab_group(&group),
            ClientAction::RequestRenameTab => {
                if self.request_rename_draft().is_some() {
                    ClientActionDisposition::DismissNavigator
                } else {
                    ClientActionDisposition::KeepNavigator
                }
            }
            ClientAction::RequestCloseTab => self.request_close_confirmation(),
            ClientAction::RequestWorkspaceClose => self.request_workspace_close(),
            ClientAction::RequestWorkspaceCloseAt(workspace) => {
                self.request_workspace_close_at(&workspace)
            }
            ClientAction::RequestSidebarWorkspaceDelete(request) => {
                self.request_sidebar_workspace_delete(&request)
            }
            ClientAction::RequestControlTakeover => self.request_takeover(),
            ClientAction::EnterResizeMode => {
                let _consumed = self.history_key(&crate::client_view::KeyEvent::press(
                    crate::client_view::KeyCode::Esc,
                    crate::client_view::Modifiers::NONE,
                ));
                ClientActionDisposition::KeepNavigator
            }
            ClientAction::ResizePane { .. } | ClientAction::ExitResizeMode => {
                ClientActionDisposition::KeepNavigator
            }
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
            ClientAction::EnterHistory => self.enter_history(),
            ClientAction::EnterCopyMode => self.enter_copy_mode(),
            ClientAction::PreviousPresentation => self.cycle_presentation(false),
            ClientAction::NextPresentation => self.cycle_presentation(true),
            ClientAction::MovePanelFocus(direction) => self.move_panel_focus(direction),
            ClientAction::FocusPanel(panel) => self.move_panel_focus_to(&panel),
            ClientAction::ToggleZoom => self.toggle_zoom(),
            ClientAction::SplitPanel(direction) => {
                self.panel_operation(PanelRequestKind::Split { direction })
            }
            ClientAction::ClosePanel => self.request_panel_close_confirmation(),
            ClientAction::MovePanel(direction) => {
                self.panel_operation(PanelRequestKind::Move { direction })
            }
            ClientAction::OpenWorkbenchNavigator => self.open_navigator(),
            ClientAction::ToggleAgentBoard => self.toggle_agent_board(),
            ClientAction::FocusBusinessSidebar | ClientAction::DismissOnboarding => {
                ClientActionDisposition::KeepNavigator
            }
            ClientAction::ReloadConfiguration
            | ClientAction::ToggleBusinessSidebar
            | ClientAction::OpenShortcutHelp
            | ClientAction::CloseShortcutHelp
            | ClientAction::PageShortcutHelp
            | ClientAction::CloseWorkbenchNavigator
            | ClientAction::TogglePointerCapture
            | ClientAction::ForwardLiteralPrefix
            | ClientAction::DismissPrefix
            | ClientAction::UnknownPrefix
            | ClientAction::Exit => {
                self.clear_notice();
                ClientActionDisposition::KeepNavigator
            }
        }
    }

    fn expire_status_notices(&mut self) -> bool {
        let Some(state) = self.state.as_mut() else {
            return false;
        };
        let now = self.clock.now();
        let notice = state.notice().filter(|notice| notice.transient());
        let setup = state.setup_status().filter(|status| status.transient());
        let notice_expired = self.notice_timer.expired(notice, now);
        let setup_expired = self.setup_notice_timer.expired(setup, now);
        if notice_expired {
            state.clear_notice();
        }
        if setup_expired {
            state.clear_settled_setup_status();
        }
        notice_expired || setup_expired
    }

    fn clear_notice(&mut self) {
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return;
        };
        mailbox.apply_local_for(state, WorkbenchClientState::clear_notice);
    }

    fn create_tab(&mut self) -> ClientActionDisposition {
        self.create_tab_placed(None)
    }

    fn create_tab_placed(
        &mut self,
        placement: Option<orchbee_workbench_protocol::TabCreationPlacement>,
    ) -> ClientActionDisposition {
        let (launch, description) = self.default_create_launch();
        self.create_tab_launch(placement, launch, description)
    }

    fn default_create_launch(
        &self,
    ) -> (
        Option<orchbee_workbench_protocol::LaunchSelection>,
        Option<super::state::LaunchDescription>,
    ) {
        let profile = self
            .preferences
            .launch_profiles
            .profiles
            .iter()
            .find(|profile| profile.id == self.preferences.launch_profiles.default_profile_id);
        let Some(profile) = profile else {
            return (None, None);
        };
        let (executable, arguments) = match &profile.command {
            orchbee_workbench_protocol::UserLaunchCommand::CustomExecutable {
                executable,
                arguments,
            } => (executable.clone(), arguments.clone()),
            orchbee_workbench_protocol::UserLaunchCommand::SystemShell { .. } => {
                // Only the executing Worker can resolve its system shell and defaults.
                // An unpinned request must not claim this Client's cached profile.
                return (None, None);
            }
        };
        let Ok(launch) = orchbee_workbench_protocol::LaunchSelection::new(executable, arguments)
        else {
            // Saved defaults have a wider value budget than explicit launches.
            // Preserve Worker resolution without claiming unpinned launch facts.
            return (None, None);
        };
        let description = super::state::LaunchDescription {
            profile_name: profile.name.clone(),
            executable: Some(launch.executable().to_owned()),
        };
        (Some(launch), Some(description))
    }

    fn create_tab_launch(
        &mut self,
        placement: Option<orchbee_workbench_protocol::TabCreationPlacement>,
        launch: Option<orchbee_workbench_protocol::LaunchSelection>,
        description: Option<super::state::LaunchDescription>,
    ) -> ClientActionDisposition {
        if self.view().no_current_workspace() {
            if placement.is_some() {
                if let Some(state) = self.state.as_mut() {
                    state.set_notice(ClientNotice::NoWorkspace);
                }
                return ClientActionDisposition::KeepNavigator;
            }
            return if self.request_quick_workspace() {
                ClientActionDisposition::WorkspaceCreationStarted
            } else {
                ClientActionDisposition::KeepNavigator
            };
        }
        if let Some(state) = &mut self.state {
            state.cancel_duplicate_sequence();
        }
        if let Some(state) = self.state.as_mut()
            && let Some(context) = state.cache().canonical_context()
            && orchbee_workbench_protocol::validate_tab_creation(context, placement.as_ref())
                == Err(orchbee_workbench_protocol::WorkbenchRejectionReason::TabMoveOrderConflict)
        {
            state.set_notice(ClientNotice::CreateOrderConflict);
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(CarrierFailure::Deadline);
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_create_for(
            state,
            |state, generation, request_id| {
                build_create_launch(
                    state,
                    generation,
                    request_id,
                    deadlines,
                    placement,
                    launch,
                    description,
                )
            },
            finish_action,
        );
        match result {
            Ok(_operation) => ClientActionDisposition::DismissNavigator,
            Err(error) => {
                self.retire_for_admission(&error);
                ClientActionDisposition::KeepNavigator
            }
        }
    }

    fn select_tab(&mut self, action: ClientAction) -> ClientActionDisposition {
        self.remember_history_positions();
        if let Some(state) = &mut self.state {
            state.clear_tab_selection();
        }
        if self.select_tab_generation(action).is_some() {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }

    fn select_tab_generation(&mut self, action: ClientAction) -> Option<u64> {
        if let Some(state) = &mut self.state {
            state.cancel_duplicate_sequence();
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(CarrierFailure::Deadline);
            return None;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return None;
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
            Ok(generation) => {
                // State hides the old projections while retaining the exact renderers for a proven restore.
                self.pending_draws.clear();
                self.ordinary_poll_serial = None;
                Some(generation)
            }
            Err(error) => {
                self.retire_for_admission(&error);
                None
            }
        }
    }

    pub(super) fn navigate_context(
        &mut self,
        target: &super::ClientContextTarget,
    ) -> super::ContextNavigationAdmission {
        self.remember_history_positions();
        use super::ContextNavigationAdmission;
        let Some(state) = self.state.as_ref() else {
            return ContextNavigationAdmission::Unavailable;
        };
        let view = self.view();
        if view.navigation_session() != Some(&target.session_id) || view.retirement().is_some() {
            return ContextNavigationAdmission::Unavailable;
        }
        let Some(index) = (0..view.context_count()).find(|index| {
            view.context_at(*index)
                .is_some_and(|context| context.location() == &target.location)
        }) else {
            return ContextNavigationAdmission::Unavailable;
        };
        // Busy precedes exact-current: A clicked during A -> B must wait to return to A.
        if !state.context_selection_available() {
            return ContextNavigationAdmission::Busy;
        }
        if view.active_context_position() == Some(index + 1) {
            return ContextNavigationAdmission::Current;
        }
        if self.select_context_at_generation(index).is_some() {
            ContextNavigationAdmission::Admitted
        } else {
            ContextNavigationAdmission::Unavailable
        }
    }

    fn select_context(&mut self, next: bool) -> ClientActionDisposition {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(CarrierFailure::Deadline);
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
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
        ClientActionDisposition::KeepNavigator
    }

    /// Pointer-issued context selection: the row index was resolved from
    /// the painted navigator, so the same refresh machinery targets it
    /// directly instead of stepping next/previous.
    fn select_context_at(&mut self, index: usize) -> ClientActionDisposition {
        self.select_context_at_generation(index);
        ClientActionDisposition::KeepNavigator
    }

    fn select_context_at_generation(&mut self, index: usize) -> Option<u64> {
        if self
            .state
            .as_ref()
            .is_some_and(|state| state.view().active_context_position() == Some(index + 1))
        {
            // Clicking the active row is a no-op, not a busy refusal.
            return None;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(CarrierFailure::Deadline);
            return None;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return None;
        };
        let result = mailbox.try_enqueue_context_selection_for(
            state,
            deadlines,
            &self.clock,
            |state, generation| {
                state
                    .start_context_selection_at(generation, index, deadlines)
                    .then_some(())
                    .ok_or(ActionBuildError::Busy)
            },
            finish_action,
        );
        match result {
            Ok(generation) => generation,
            Err(error) => {
                self.retire_for_admission(&error);
                None
            }
        }
    }

    /// Pointer-issued panel focus: the pointer resolved a concrete stable
    /// panel identity in the active tab; the admission path is the same
    /// purely local transition as the directional `move_panel_focus` — the
    /// pane's viewer is already live under the same anchor, so no
    /// detach/attach round trip is minted.
    fn move_panel_focus_to(&mut self, panel: &WorkbenchPanelId) -> ClientActionDisposition {
        self.move_panel_focus_admitted(panel)
            .unwrap_or(ClientActionDisposition::KeepNavigator)
    }

    fn move_panel_focus_admitted(
        &mut self,
        panel: &WorkbenchPanelId,
    ) -> Option<ClientActionDisposition> {
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return None;
        };
        let pane = state.plan_panel_focus_local_at(panel)?;
        if !state.move_panel_focus_local(pane.clone()) {
            self.retire(CarrierFailure::Protocol);
            return None;
        }
        // Under zoom the drawn set follows the focus: the pointer map and
        // the observed geometry are stale the moment the move lands.
        let zoomed = state.zoom_active();
        if let Err(closed) = mailbox.publish_desired(pane) {
            self.retire(closed.failure());
            return None;
        }
        if zoomed {
            return Some(ClientActionDisposition::LayoutChanged);
        }
        Some(ClientActionDisposition::KeepNavigator)
    }

    /// `Ctrl+B z`: the volatile Client-local zoom toggle. The mailbox lock
    /// serializes flag minting against membership installs so a flag can
    /// never be minted on a Tab an in-flight install is shrinking to one
    /// leaf — the same TOCTOU the local confirmations close.
    fn toggle_zoom(&mut self) -> ClientActionDisposition {
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        if !mailbox.apply_local_for(state, WorkbenchClientState::toggle_zoom) {
            return ClientActionDisposition::KeepNavigator;
        }
        ClientActionDisposition::LayoutChanged
    }

    fn move_tab(&mut self, direction: WorkbenchTabMoveDirection) -> ClientActionDisposition {
        // Capture the entire transport budget before attempting the admission mutex.
        let Some(deadline) = self.clock.now().checked_add(super::state::OPERATION_TOTAL) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
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
            Ok(_) => ClientActionDisposition::DismissNavigator,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                ClientActionDisposition::KeepNavigator
            }
            // A deadline refusal can return while another thread still holds the mailbox.
            // Preserve local state and do not reacquire that lock to update a notice.
            Err(_) => ClientActionDisposition::KeepNavigator,
        }
    }

    fn panel_operation(&mut self, kind: PanelRequestKind) -> ClientActionDisposition {
        if self.panel_close_confirmation_armed() {
            return ClientActionDisposition::KeepNavigator;
        }
        // Capture the entire transport budget before attempting the admission mutex.
        let Some(deadline) = self.clock.now().checked_add(super::state::OPERATION_TOTAL) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        state.configure_panel_focus(self.focus_new_panel);
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
            Ok(_) => ClientActionDisposition::DismissNavigator,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                ClientActionDisposition::KeepNavigator
            }
            Err(_) => ClientActionDisposition::KeepNavigator,
        }
    }

    /// A directional focus move is a pure local transition: the neighbour
    /// pane's viewer is already live under the same anchor, so no
    /// detach/attach round trip is minted. The mailbox's desired key is
    /// republished so focus-scoped admission (input/resize) follows the move.
    fn move_panel_focus(&mut self, direction: PanelDirection) -> ClientActionDisposition {
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        let Some(pane) = state.plan_panel_focus_local(direction) else {
            return ClientActionDisposition::KeepNavigator;
        };
        if !state.move_panel_focus_local(pane.clone()) {
            self.retire(CarrierFailure::Protocol);
            return ClientActionDisposition::KeepNavigator;
        }
        // Under zoom the drawn set follows the focus: the pointer map and
        // the observed geometry are stale the moment the move lands.
        let zoomed = state.zoom_active();
        if let Err(closed) = mailbox.publish_desired(pane) {
            self.retire(closed.failure());
        }
        if zoomed {
            return ClientActionDisposition::LayoutChanged;
        }
        ClientActionDisposition::KeepNavigator
    }

    fn open_navigator(&mut self) -> ClientActionDisposition {
        if self
            .state
            .as_ref()
            .is_some_and(WorkbenchClientState::setup_recovery)
        {
            return self.refresh_workspace_setup();
        }
        let mut reconcile = false;
        if let Some(state) = &mut self.state {
            state.request_close_manual_pass();
            if state.close_status_only() {
                reconcile = state.request_close_membership_recovery().is_ok();
                if !reconcile {
                    return ClientActionDisposition::KeepNavigator;
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
            return ClientActionDisposition::KeepNavigator;
        }
        let pending = {
            let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
                return ClientActionDisposition::KeepNavigator;
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
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(deadline) = self.clock.now().checked_add(super::state::OPERATION_TOTAL) else {
            self.retire(CarrierFailure::Deadline);
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
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
        ClientActionDisposition::KeepNavigator
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
        if wait_key.is_none() {
            if let WorkerEvent::SelectionOutcome {
                key,
                command,
                outcome,
            } = event
            {
                return self.apply_selection_outcome(mailbox, key, command, outcome);
            }
            // Viewport outcomes are fire-and-forget publications: a stale
            // incarnation's outcome (or one for a core that already retired)
            // drops silently — unlike pane facts it never diverges
            // canonical state, so a drop is not a protocol fault.
            if let WorkerEvent::ViewportOutcome { key, outcome } = event {
                return self.apply_viewport_outcome(mailbox, key, outcome);
            }
            // Notice events carry no wait key: they are canonical pane-fact
            // publications that settle state directly instead of depositing a
            // causal reply.
            let applied = match event {
                WorkerEvent::PaneDetached { key } => {
                    self.clear_pane_projection(&key);
                    self.state
                        .as_mut()
                        .is_some_and(|state| state.settle_pane_detached(&key))
                }
                WorkerEvent::PaneFailed { key, fault } => {
                    if self.state.as_ref().is_some_and(|state| {
                        state
                            .pane_record(key.pane())
                            .is_none_or(|record| record.pane_ref() != key)
                    }) {
                        // The exact incarnation already departed; a late loss fact
                        // cannot retire the selection that replaced it.
                        return false;
                    }
                    let applied = self
                        .state
                        .as_mut()
                        .is_some_and(|state| state.settle_pane_fault(&key, fault));
                    if applied && fault == super::state::PaneFault::ViewerLost {
                        self.clear_pane_projection(&key);
                    }
                    applied
                }
                _ => false,
            };
            if !applied {
                self.retire(CarrierFailure::Protocol);
                return true;
            }
            return self.validate_state();
        }
        let Some(wait_key) = wait_key else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        if let WorkerEvent::Bootstrap { cache, selection } = event {
            if self.state.is_some() {
                self.retire(CarrierFailure::Protocol);
                return true;
            }
            let mut installed = None;
            let result = mailbox.deposit_bootstrap_reply_before(
                &wait_key,
                &self.clock,
                |generation, geometry| {
                    let (state, target) = WorkbenchClientState::bootstrap_at(
                        cache,
                        generation,
                        geometry,
                        selection.as_ref(),
                    )
                    .ok_or(CarrierFailure::AuthorityDivergence)?;
                    installed = Some(state);
                    Ok(target)
                },
            );
            match result {
                Ok(MutationResult::Applied) => {
                    self.state = installed;
                    // The accepted bootstrap seeds the board's index —
                    // only installed canonical state may feed it.
                    self.agent_index = super::agent_board::PresentationIndex::default();
                    if let Some(state) = self.state.as_ref() {
                        self.agent_index.install_context(state.cache());
                    }
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
            WorkerEvent::WorktreePrepared {
                preparation,
                source,
                outcome,
            } => {
                return self.apply_worktree_preparation(
                    mailbox,
                    wait_key,
                    *preparation,
                    source,
                    *outcome,
                );
            }
            WorkerEvent::QuickWorkspacePrepared {
                preparation,
                cwd,
                outcome,
            } => {
                return self.apply_quick_preparation_event(
                    mailbox,
                    wait_key,
                    *preparation,
                    cwd,
                    *outcome,
                );
            }
            WorkerEvent::WorkspaceSetupResult { operation, outcome } => {
                return self.apply_workspace_setup_event(mailbox, wait_key, operation, outcome);
            }
            WorkerEvent::WorkspaceCloseResult { evidence, outcome } => {
                return self.apply_workspace_close_event(mailbox, wait_key, evidence, *outcome);
            }
            WorkerEvent::ControlTakeoverResult { evidence } => {
                return self.apply_takeover_event(mailbox, wait_key, evidence);
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
        let mut clear_pane_key = None;
        let mut install_projection = None;
        let result = mailbox.deposit_ui_reply_before(&wait_key, &self.clock, |transaction| {
            let reply = match event {
                WorkerEvent::WorktreePrepared { .. } => return Err(CarrierFailure::Protocol),
                WorkerEvent::QuickWorkspacePrepared { .. }
                | WorkerEvent::WorkspaceSetupResult { .. }
                | WorkerEvent::WorkspaceCloseResult { .. }
                | WorkerEvent::ControlTakeoverResult { .. }
                | WorkerEvent::RenameResult { .. }
                | WorkerEvent::MoveResult { .. }
                | WorkerEvent::PanelResult { .. }
                | WorkerEvent::CloseReconciliationSnapshot { .. }
                | WorkerEvent::MembershipGone { .. }
                | WorkerEvent::MembershipDetached { .. }
                | WorkerEvent::CloseResult { .. }
                | WorkerEvent::SwitchSnapshotResult { .. }
                | WorkerEvent::MembershipCleanupSkipped { .. }
                | WorkerEvent::ViewportOutcome { .. }
                | WorkerEvent::SelectionOutcome { .. } => {
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
                    clear_pane_key = Some(key);
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
                    if state.duplicate_owns_operation(&operation) {
                        state.clear_notice();
                    } else {
                        state.set_notice(notice);
                    }

                    if matches!(
                        state.controller_mode(),
                        Some(ControllerMode::Controlling(_))
                    ) && Some(transaction.latest_geometry()) != applied_geometry
                    {
                        let desired = state.focus_pane_ref().ok_or(CarrierFailure::Protocol)?;
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
                            effect_stream: super::terminal_effects::Stream::new(
                                *projection.attachment(),
                                *projection.epoch(),
                            ),
                            key,
                            value: projection,
                            status: ClientProjectionStatus::Empty,
                            viewport: None,
                            tail: None,
                            tail_attempted: None,
                            history_sequence: 0,
                            read_watermark: 0,
                            selection: None,
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
                    clear_pane_key = Some(key);
                    UiReply::CleanupApplied(continuation)
                }
                WorkerEvent::Bootstrap { .. }
                | WorkerEvent::RefreshSnapshotResult { .. }
                | WorkerEvent::PollResult { .. }
                | WorkerEvent::PaneDetached { .. }
                | WorkerEvent::PaneFailed { .. } => {
                    return Err(CarrierFailure::Protocol);
                }
            };
            Ok(reply)
        });
        match result {
            Ok(MutationResult::Applied) => {
                if let Some(key) = clear_pane_key {
                    self.clear_pane_projection(&key);
                }
                if let Some(projection) = install_projection {
                    self.projections
                        .retain(|existing| existing.key != projection.key);
                    self.projections.push(projection);
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
        if self.retired_peer_attempt.is_some() {
            return self.apply_retired_membership_result(mailbox, wait_key, generation, outcome);
        }
        let Some(state) = self.state.as_ref() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        // The board's presentation index may only consume a snapshot the
        // session itself accepted — keep it until the verdict proves the
        // snapshot drove a Membership/Append application.
        let index_snapshot = match &outcome {
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available { snapshot } => {
                Some(snapshot.clone())
            }
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Denied { .. }
            | orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Unavailable { .. } => None,
        };
        // Projection and append-only validation are bounded but intentionally run outside the
        // mailbox mutex. Only the prepared value is committed in the exact reply transaction.
        let prepared = state.prepare_refresh(generation, outcome);
        let prepared = self.fence_worktree_context_candidate(
            prepared,
            index_snapshot.as_ref(),
            generation,
            state.cache().revision(),
        );
        let prepared =
            self.fence_menu_context_candidate(prepared, generation, state.cache().revision());
        let menu_membership = matches!(prepared, super::state::PreparedRefresh::Membership { .. });
        let recovery_deadlines = state
            .workspace_close_deadlines()
            .or_else(|| state.setup_deadlines())
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
                index_snapshot,
            );
        }
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        // Only a snapshot that actually drove the accepted refresh may
        // rebuild the index — a Stale, DefiniteFailure or Divergent
        // verdict never feeds it.
        let rebuild_index = index_snapshot.is_some()
            && matches!(
                prepared,
                super::state::PreparedRefresh::Membership { .. }
                    | super::state::PreparedRefresh::Append { .. }
            );
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
                if state.has_attachments() && !prepared.preserves_selection() {
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
                    match prepared.focus_panel_id() {
                        Some(panel) => super::mailbox::DesiredInstall::Fresh {
                            tab_id: tab.clone(),
                            panel: panel.clone(),
                        },
                        None => super::mailbox::DesiredInstall::Empty,
                    }
                } else {
                    super::mailbox::DesiredInstall::Empty
                };
                let desired_plan = transaction.prepare_desired_install(install)?;
                let state_plan = state.prepare_refresh_membership_install(
                    generation,
                    prepared,
                    desired_plan.desired().map(|key| key.anchor().clone()),
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
                state.apply_prepared_refresh(prepared, transaction.observed_at())
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
                {
                    self.menu_navigation_prepared(
                        action_menus::NavigationKey::Context(generation),
                        menu_membership,
                    );
                    if menu_membership
                        && self.state.as_ref().is_some_and(|s| !s.context_selecting())
                    {
                        self.menu_navigation_installed(action_menus::NavigationKey::Context(
                            generation,
                        ));
                    }
                }
                if rebuild_index && let Some(snapshot) = &index_snapshot {
                    self.observe_worktree_entry_snapshot(snapshot);
                    self.agent_index = super::agent_board::PresentationIndex::rebuild(snapshot);
                    if let Some(state) = self.state.as_mut() {
                        state.observe_overlay_snapshot(snapshot);
                    }
                }
                if clear_projection {
                    self.clear_projections();
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
                // Empty replies wake the UI too. Pace their successors so an
                // idle terminal cannot turn the notification path into a spin.
                self.next_empty_poll = self
                    .clock
                    .now()
                    .checked_add(std::time::Duration::from_millis(8));
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
            Some(state) => state.expected_frame_geometry(poll_key.pane_ref()),
            None => {
                self.retire(CarrierFailure::Protocol);
                return true;
            }
        };
        // The poll's exact incarnation must still own an installed projection;
        // a detached or re-incarnated pane's late result is dropped, not
        // applied — the causal settlement below still answers the lease.
        let Some(index) = self
            .projections
            .iter()
            .position(|projection| &projection.key == poll_key.pane_ref())
        else {
            self.pending_draws.push(PendingDraw {
                poll_key,
                settlement: CausalSettlement::NoFeedback,
                frame: None,
                ordinary,
            });
            return true;
        };
        match self.apply_program_effect(index, &record) {
            Ok(true) => {
                self.pending_draws.push(PendingDraw {
                    poll_key,
                    settlement: CausalSettlement::NoFeedback,
                    frame: None,
                    ordinary,
                });
                return true;
            }
            Ok(false) => {}
            Err(()) => {
                self.retire(CarrierFailure::Protocol);
                return true;
            }
        }
        match self.apply_selection_event(index, &record) {
            Ok(true) => {
                self.pending_draws.push(PendingDraw {
                    poll_key,
                    settlement: CausalSettlement::NoFeedback,
                    frame: None,
                    ordinary,
                });
                return true;
            }
            Ok(false) => {}
            Err(failure) => {
                self.settle_selection_failure(failure);
                return true;
            }
        }
        match self.projections[index].value.receive_lifecycle(&record) {
            Ok(true) => {
                if self.projections[index].value.end().is_some()
                    && let Some(state) = &mut self.state
                {
                    state.settle_pane_fault(
                        poll_key.pane_ref(),
                        super::state::PaneFault::ControlLost,
                    );
                }
                self.pending_draws.push(PendingDraw {
                    poll_key,
                    settlement: CausalSettlement::NoFeedback,
                    frame: None,
                    ordinary,
                });
                return true;
            }
            Ok(false) => {}
            Err(()) => {
                self.retire(CarrierFailure::Protocol);
                return true;
            }
        }
        let result = self.projections[index]
            .value
            .receive_for_geometry(&record, expected);
        let (settlement, frame, status) = match result {
            Ok(Some(feedback @ TerminalViewerAction::HistoryObserved { .. })) => {
                // kind-53 intake — the unique S1 sink: the projection owns
                // envelope/attachment validation and mints the single
                // `HistoryObserved` acknowledgement; every state — Live
                // included — acks exactly once through the ordinary causal
                // lane. S2 merges its consumption here: the same validated
                // event feeds the viewport core, strictly in sequence.
                match decode_history_changed(
                    &record,
                    self.projections[index].value.attachment(),
                    self.projections[index].value.epoch(),
                ) {
                    Ok(Some(changed)) => {
                        if changed.sequence.get() <= self.projections[index].history_sequence {
                            // Kind-53 sequences are strictly monotonic per
                            // incarnation: a repeat or regression makes the
                            // stream undecidable.
                            self.retire(CarrierFailure::Protocol);
                            return true;
                        }
                        self.projections[index].history_sequence = changed.sequence.get();
                        if let Some(viewport) = self.projections[index].viewport.as_mut() {
                            let effect = viewport.facts(changed);
                            self.settle_viewport_effect(mailbox, poll_key.pane_ref(), effect);
                            if self.retirement_failure.is_some() {
                                return true;
                            }
                        }
                    }
                    // The projection already accepted this event, so S2's
                    // identical decode can only succeed; anything else is a
                    // fault, not a silent skip.
                    _ => {
                        self.retire(CarrierFailure::Protocol);
                        return true;
                    }
                }
                (
                    CausalSettlement::Feedback(feedback),
                    None,
                    self.projections[index].status,
                )
            }
            Ok(Some(feedback @ TerminalViewerAction::FrameApplied { .. })) => (
                CausalSettlement::Feedback(feedback),
                Some(FrameIdentity::new(poll_key.serial())),
                ClientProjectionStatus::FrameAvailable,
            ),
            Ok(Some(feedback @ TerminalViewerAction::NeedSnapshot { .. })) => {
                if self
                    .state
                    .as_mut()
                    .is_none_or(|state| !state.invalidate_projection_frame(poll_key.pane_ref()))
                {
                    self.retire(CarrierFailure::Protocol);
                    return true;
                }
                (
                    CausalSettlement::Feedback(feedback),
                    None,
                    ClientProjectionStatus::WaitingForFullFrame,
                )
            }
            Ok(None) => {
                if self
                    .state
                    .as_mut()
                    .is_none_or(|state| !state.invalidate_projection_frame(poll_key.pane_ref()))
                {
                    self.retire(CarrierFailure::Protocol);
                    return true;
                }
                (
                    CausalSettlement::NoFeedback,
                    None,
                    ClientProjectionStatus::Blocked,
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
        self.projections[index].status = status;
        if status == ClientProjectionStatus::Blocked
            && let Some(state) = self.state.as_ref()
        {
            self.retained_frames
                .invalidate(state, poll_key.pane_ref().pane().panel());
        }
        if frame.is_some() {
            // A settled live frame ends an admitted recovery, and a
            // host-side geometry change inside that frame re-geometries a
            // surviving core. Only a real FrameApplied arm reaches here —
            // kind-53 events never re-run frame hooks.
            if let Some(viewport) = self.projections[index].viewport.as_mut() {
                let effect = viewport.frame_applied();
                self.settle_viewport_effect(mailbox, poll_key.pane_ref(), effect);
            }
            if self.retirement_failure.is_none() {
                let geometry = self
                    .projections
                    .get(index)
                    .and_then(|projection| projection.value.frame())
                    .map(|frame| (frame.geometry.columns, frame.geometry.rows));
                if let Some((columns, rows)) = geometry
                    && let Some(viewport) = self
                        .projections
                        .get_mut(index)
                        .and_then(|projection| projection.viewport.as_mut())
                    && viewport.geometry() != (columns, rows)
                {
                    let effect = viewport.resize(columns, rows, self.clock.now());
                    self.settle_viewport_effect(mailbox, poll_key.pane_ref(), effect);
                }
            }
        }
        self.pending_draws.push(PendingDraw {
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
            mailbox.capture_publication_hint_for(state, WorkbenchClientState::note_publication_hint)
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

    /// Schedules the next ordinary poll for the next pane in rotation. One
    /// ordinary poll stays in flight at a time — the single causal cell is
    /// preserved — while the cursor walks the projected pane set so every
    /// pane's viewer advances without starving the focus.
    fn schedule_poll(&mut self, mailbox: &Arc<CommandMailbox>) {
        // Existing polls/replies still drain. Do not put a fresh ordinary Poll ahead of
        // an already Ready next member while the preceding lease is finishing.
        if self
            .state
            .as_ref()
            .is_some_and(WorkbenchClientState::duplicate_waiting)
        {
            return;
        }
        if !self.pending_draws.is_empty() || self.ordinary_poll_serial.is_some() {
            return;
        }
        if self.next_empty_poll.is_some_and(|at| self.clock.now() < at) {
            return;
        }
        self.next_empty_poll = None;
        let Some(state) = self.state.as_ref() else {
            return;
        };
        if state.ready().is_none()
            && !state.focused_viewer_lost()
            && !(state.phase_kind() == ClientPhaseKind::SelectedWithoutAttachment
                && state.selected_is_unbound())
        {
            return;
        }
        let pollable = state.pollable_panes();
        if pollable.is_empty() {
            self.poll_cursor = 0;
            return;
        }
        // Only panes this UI actually projects are served — a state-side live
        // pane whose projection was never installed here (or was already
        // dropped) must not mint a poll it cannot settle.
        let start = self.poll_cursor % pollable.len();
        let key = (0..pollable.len())
            .map(|offset| &pollable[(start + offset) % pollable.len()])
            .find(|key| {
                self.projections
                    .iter()
                    .any(|projection| projection.key == **key)
            })
            .cloned();
        let Some(key) = key else {
            self.poll_cursor = 0;
            return;
        };
        self.poll_cursor = self.poll_cursor.wrapping_add(1);
        if state.close_suppresses_attachment(key.pane().anchor()) {
            return;
        }
        match mailbox.try_enqueue(QueuedCommand::Poll(Tagged::new(key, ()))) {
            Ok(serial) => {
                self.ordinary_poll_serial = Some(serial);
                self.next_empty_poll = None;
            }
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

    /// Drops every projection and staged draw — used on epoch teardown, where
    /// all panes retire together, and on retirement.
    fn clear_projections(&mut self) {
        self.history_positions.projections_cleared();
        self.copy_mode.cancel();
        self.effects.close();
        self.terminal_effect.take();
        self.projections.clear();
        self.pending_draws.clear();
        self.ordinary_poll_serial = None;
        self.poll_cursor = 0;
        self.next_empty_poll = None;
    }

    /// Drops one pane's projection and staged draws — focus-scoped settle
    /// events (`CleanupSettled`, `SetupWithoutAttachment`) and pane-local
    /// disposal land here, leaving sibling panes untouched.
    fn clear_pane_projection(&mut self, key: &PaneRef) {
        if self.copy_mode.key() == Some(key) {
            self.cancel_copy_mode();
        }
        self.effects.cancel_pane(key);
        self.discard_cancelled_terminal_effect();
        self.projections.retain(|projection| &projection.key != key);
        self.pending_draws
            .retain(|pending| pending.poll_key.pane_ref() != key);
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
        self.retained_frames.clear();
        {
            self.history_positions = history_positions::Coordinator::default();
        }
        {
            self.cancel_copy_mode();
            self.fail_action_menu();
        }
        self.effects.close();
        self.terminal_effect.take();
        self.clipboard.close();
        self.settle_effect_delivery();
        if self.retirement_failure.is_none() {
            self.retirement_poll_serial = self.ordinary_poll_serial;
        }
        self.board.close();
        self.reports.close();
        self.graphics.close();
        self.overlays.close();
        self.links.close();
        self.copy_mode.close();
        self.popup.shutdown();
        if let Some(state) = &mut self.state {
            state.close_carrier_retired();
            state.stop_duplicate_sequence(super::state::DuplicateStop::Recovery);
            state.cancel_panel_close_confirmation();
            state.cancel_workspace_close();
            state.cancel_rename_draft();
            state.cancel_setup_on_retirement();
        }
        let failure = self
            .mailbox
            .as_ref()
            .map_or(failure, |mailbox| mailbox.close(failure).failure());
        if self.retirement_failure.is_none() {
            self.retirement_failure = Some(failure);
        }
        self.clear_projections();
    }
}

const fn create_precommit_notice(outcome: &CreatePrecommitOutcome) -> ClientNotice {
    match outcome {
        CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Rejected {
            reason: orchbee_workbench_protocol::WorkbenchRejectionReason::TabMoveOrderConflict,
            ..
        }) => ClientNotice::CreateOrderConflict,
        CreatePrecommitOutcome::NotSubmitted
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Rejected { .. })
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Denied)
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Unavailable { .. })
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::ProfileUnavailable)
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::ProfileAmbiguous {
            ..
        }) => ClientNotice::CreateNotCreated,
    }
}

fn build_create(
    state: &mut WorkbenchClientState,
    generation: u64,
    request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    deadlines: OperationDeadlines,
    placement: Option<orchbee_workbench_protocol::TabCreationPlacement>,
) -> Result<CreateMarker, ActionBuildError> {
    build_create_launch(
        state, generation, request_id, deadlines, placement, None, None,
    )
}

fn build_create_launch(
    state: &mut WorkbenchClientState,
    generation: u64,
    request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    deadlines: OperationDeadlines,
    placement: Option<orchbee_workbench_protocol::TabCreationPlacement>,
    launch: Option<orchbee_workbench_protocol::LaunchSelection>,
    description: Option<super::state::LaunchDescription>,
) -> Result<CreateMarker, ActionBuildError> {
    if !state.invariant_holds() {
        return Err(ActionBuildError::Protocol);
    }
    orchbee_workbench_protocol::validate_tab_creation(
        state
            .cache()
            .canonical_context()
            .ok_or(ActionBuildError::Busy)?,
        placement.as_ref(),
    )
    .map_err(|_| ActionBuildError::Busy)?;
    let label_number = state
        .cache()
        .tabs()
        .len()
        .checked_add(1)
        .ok_or(ActionBuildError::Protocol)?;
    let mut label = WorkbenchLabel::parse(&label_number.to_string())
        .map_err(|_error| ActionBuildError::Protocol)?;
    let mut automatic_label = true;
    if let Some(orchbee_workbench_protocol::TabCreationPlacement::Duplicate { tab_id }) = &placement
    {
        let source = state
            .cache()
            .canonical_context()
            .ok_or(ActionBuildError::Busy)?
            .tabs
            .iter()
            .find(|tab| &tab.tab_id == tab_id)
            .ok_or(ActionBuildError::Busy)?;
        label = WorkbenchLabel::parse(&source.label).map_err(|_| ActionBuildError::Protocol)?;
        automatic_label = source.automatic_label;
    }
    let geometry = state.latest_geometry();
    let workbench_geometry = WorkbenchTerminalGeometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry.cell_pixel_width(),
        geometry.cell_pixel_height(),
    )
    .map_err(|_error| ActionBuildError::Protocol)?;
    let key = CreateOperationKey::new(request_id.clone(), generation);
    let mut operation = CreateOperation::new(
        key,
        state.cache().session_id().clone(),
        state.cache().revision(),
        state
            .cache()
            .terminal_context_id()
            .ok_or(ActionBuildError::Busy)?
            .clone(),
        label.clone(),
        geometry,
        deadlines,
    )
    .with_placement(placement.clone())
    .with_launch_description(description)
    .with_origin(if state.phase_kind() == ClientPhaseKind::Empty {
        super::state::CreateOriginKind::Empty
    } else if matches!(
        state.phase_kind(),
        ClientPhaseKind::SelectedWithoutAttachment | ClientPhaseKind::RetryableAttachUnavailable
    ) {
        super::state::CreateOriginKind::Unattached
    } else {
        super::state::CreateOriginKind::Ready
    });
    if automatic_label {
        operation = operation.with_automatic_label();
    }
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id,
        session_id: state.cache().session_id().clone(),
        expected_revision: state.cache().revision(),
        action: WorkbenchAction::TabCreate {
            placement,
            automatic_label,
            terminal_context_id: state
                .cache()
                .terminal_context_id()
                .ok_or(ActionBuildError::Busy)?
                .clone(),
            label,
            geometry: workbench_geometry,
            launch,
            profile: None,
            options: Default::default(),
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
    state: &mut WorkbenchClientState,
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

/// `build_switch` carrying the stable panel identity the Agent board's
/// jump wants focused after the tab switch lands — the same admission
/// checks, the same `begin_switch`, only the selected pane differs: it
/// resolves `panel_id` to its current index instead of `panel(0)`.
fn build_switch_for_panel(
    state: &mut WorkbenchClientState,
    action: ClientAction,
    panel_id: &orchbee_workbench_protocol::WorkbenchPanelId,
    generation: u64,
    deadlines: OperationDeadlines,
) -> Result<SwitchMarker, ActionBuildError> {
    if !state.invariant_holds() {
        return Err(ActionBuildError::Protocol);
    }
    let index = match state.plan_selection(action) {
        SelectionPlan::Busy => return Err(ActionBuildError::Busy),
        SelectionPlan::NoOp => {
            // Same-tab target the ordinary planner calls a NoOp: for the
            // jump it may still mean "reattach a dead viewer on this
            // tab". The state re-checks every guard — a live target
            // stays a NoOp, and only a genuinely dead panel mints a
            // fresh-generation attach on the same anchor.
            let Some(target) =
                state.target_for_dead_panel_reattach(panel_id, generation, deadlines)
            else {
                return Err(ActionBuildError::NoOp);
            };
            if !state.begin_switch(target.clone()) {
                return Err(ActionBuildError::Protocol);
            }
            return Ok(SwitchMarker::new(target));
        }
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
        .target_for_switch_for_panel(index, key, deadlines, panel_id)
        .ok_or(ActionBuildError::Protocol)?;
    if !state.begin_switch(target.clone()) {
        return Err(ActionBuildError::Protocol);
    }
    Ok(SwitchMarker::new(target))
}

fn finish_action<Value>(
    state: &mut WorkbenchClientState,
    result: &Result<Value, AdmissionError<ActionBuildError>>,
) {
    match result {
        Ok(_) | Err(AdmissionError::Build(ActionBuildError::NoOp)) => state.clear_notice(),
        Err(AdmissionError::Build(ActionBuildError::OutOfRange(position))) => {
            state.set_notice(ClientNotice::NoTabAtPosition(*position));
        }
        Err(
            AdmissionError::BootstrapPending
            | AdmissionError::Busy
            | AdmissionError::Full
            | AdmissionError::StaleAttachment
            | AdmissionError::Build(ActionBuildError::Busy),
        ) => state.set_notice(ClientNotice::LocalBusy),
        Err(AdmissionError::Closed(_))
        | Err(AdmissionError::AttachmentGenerationExhausted)
        | Err(AdmissionError::RefreshGenerationExhausted)
        | Err(AdmissionError::Build(ActionBuildError::Protocol)) => {}
    }
}
