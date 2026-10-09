#[cfg(windows)]
mod action_menus;
#[cfg(windows)]
mod agent_attention;
#[cfg(windows)]
mod pane_graphics;
#[cfg(windows)]
pub(in crate::terminal_session) mod settings;
#[cfg(windows)]
mod tab_groups;
#[cfg(all(windows, test))]
pub(in crate::terminal_session) use agent_attention::tests::focused_physical_agent_fixture;
#[cfg(windows)]
mod agent_focus;
#[cfg(windows)]
mod client_control;
#[cfg(windows)]
mod client_layout;
use super::terminal_viewport::ViewportWindow;
use super::{
    mailbox::{
        AdmissionError, CausalSettlement, CommandMailbox, CreateMarker, CreatePrecommitOutcome,
        EventError, MoveAdmissionError, MutationResult, QueuedCommand, RefreshSettlement,
        SwitchMarker, Tagged, UiReply, ViewportOutcome, WorkerEvent,
    },
    state::{
        AttachmentKey, CarrierFailure, ControllerMode, CreateOperation, CreateOperationKey,
        CreateProjectionDecision, CreateTerminalProvenance, FrameIdentity, NativeSessionState,
        NativeSessionView as StateSessionView, NativeTabView as StateTabView,
        NavigatorRefreshStatus, OperationDeadlines, PaneKey, PaneRef, PanelRequestKind,
        PendingCreateCommit, PendingMoveCommit, PendingPanelCommit, RefreshDecision, SelectionPlan,
        SessionNotice, SessionPhaseKind, SetupEventDisposition,
    },
    terminal_viewport::{
        TerminalViewport, ViewportEffect, ViewportNav, ViewportStep, ViewportSurface,
        decode_history_changed,
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
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchTabId, WorkbenchTabMoveDirection, WorkbenchTerminalGeometry,
};
use std::sync::Arc;

mod agent_board;
mod close_reconciliation;
mod control_takeover;
mod membership;
mod move_membership;
#[cfg(windows)]
mod navigator;
mod panel;
mod panel_close;
mod presentations;
mod retained_frames;
#[cfg(any(target_os = "linux", windows))]
mod retired_membership;
mod tab_close;
#[cfg(test)]
mod tests;
mod workspace_close;
mod workspace_setup;
#[cfg(windows)]
mod worktree_launch;
pub(crate) use workspace_close::{
    WorkspaceCloseConfirmationView, WorkspaceCloseStatus, WorkspaceCloseStatusView,
    WorkspaceCloseToken,
};
pub(crate) use workspace_setup::{
    NativeLocationLabelStatus, NativeLocationLabelTarget, NativeSetupDraftView, NativeSetupEdit,
    NativeSetupInputContext, NativeSetupStage, NativeSetupStatus, NativeSetupToken,
};
#[cfg(windows)]
pub(crate) use worktree_launch::{
    WorktreeEdit, WorktreeInputContext, WorktreeMode, WorktreeNotice, WorktreeStage, WorktreeToken,
    WorktreeView,
};
#[cfg(windows)]
mod copy_mode;
#[cfg(test)]
mod program_effect_tests;
#[cfg(test)]
mod selection_tests;
mod tab_rename;
mod terminal_effects;
mod terminal_selection;
pub(crate) use panel_close::{NativePanelCloseConfirmationToken, NativePanelCloseConfirmationView};
pub(crate) use tab_close::{
    ModalPresentation, NativeCloseConfirmationKind, NativeCloseConfirmationToken,
    NativeCloseConfirmationView, NativeCloseStatusKind, NativeCloseStatusToken,
    NativeCloseStatusView, NativeDrawReceipt,
};
pub(crate) use tab_rename::{
    NativeRenameDraftToken, NativeRenameDraftView, NativeRenameEdit, NativeRenameInputContext,
};
mod create_membership;
#[cfg(windows)]
mod startup_preference;
mod switch_membership;

pub(crate) use super::agent_board::{
    AgentBoardDraw, AgentBoardRead, AgentBoardReadFailure, AgentBoardRow, AgentBoardSummary,
    JumpFailure, NativeAgentBoardView, PresentationPlacement,
};

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
    #[cfg(windows)]
    Settings(settings::Notice),
    #[cfg(windows)]
    MenuUnavailable,
    LocalBusy,
    NoTabAtPosition(u8),
    CreateNotCreated,
    DuplicatePending,
    DuplicateNotStarted,
    DuplicateOutcomeUnknown,
    MoveNotMoved,
    MoveOrderConflict,
    PanelNotApplied,
    RenameNotRenamed,
    CloseRequestNotApplied,
    WorkspaceCloseUnavailable,
    ConfirmationCancelled,
    DraftCancelled,
    ModalOwnsInput,
    HistoryRecovering,
    HistoryUnavailable,
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
    #[cfg(windows)]
    PositionChanged,
    #[cfg(windows)]
    PositionUnavailable,
    #[cfg(windows)]
    PopupUnavailable,
    #[cfg(windows)]
    CommandAccepted,
    #[cfg(windows)]
    CommandFailed,
    #[cfg(windows)]
    CommandUnknown,
    #[cfg(windows)]
    LinkOpened,
    #[cfg(windows)]
    LinkHandled,
    #[cfg(windows)]
    LinkNoLink,
    #[cfg(windows)]
    LinkBusy,
    #[cfg(windows)]
    LinkFailed,
    #[cfg(windows)]
    LinkUnknown,
}

#[cfg(windows)]
const fn link_notice(notice: crate::terminal_link_activation::Notice) -> NativeSessionNotice {
    match notice {
        crate::terminal_link_activation::Notice::Opened => NativeSessionNotice::LinkOpened,
        crate::terminal_link_activation::Notice::Handled => NativeSessionNotice::LinkHandled,
        crate::terminal_link_activation::Notice::NoLink => NativeSessionNotice::LinkNoLink,
        crate::terminal_link_activation::Notice::Failed => NativeSessionNotice::LinkFailed,
        crate::terminal_link_activation::Notice::Unknown => NativeSessionNotice::LinkUnknown,
    }
}

/// What one pane's retained-history surface paints right now. `Window`
/// hands out a row window the core re-validated against current geometry
/// and facts; the other cases overlay their own indicator and never hand
/// out stale row data.
#[derive(Clone, Copy, Debug)]
pub(crate) enum NativeViewportSurface<'a> {
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
        self.inner.split_ratio(split, first, second)
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

    /// The tab's stable identity — recorded into pointer hit regions so a
    /// click resolves the entry, never a stale position.
    pub(crate) fn tab_id(&self) -> &'a WorkbenchTabId {
        self.inner.tab_id()
    }
}

pub(crate) struct NativeSessionView<'a> {
    #[cfg(windows)]
    settings: Option<&'a settings::Coordinator>,
    #[cfg(windows)]
    menu: Option<&'a crate::action_menu::ActionMenu>,
    state: Option<StateSessionView<'a>>,
    panes: &'a [KeyedProjection],
    retained_frames: Option<retained_frames::RetainedView<'a>>,
    focus: Option<&'a WorkbenchPanelId>,
    projection_status: NativeProjectionStatus,
    retirement: Option<NativeDisplayRetirement>,
    board: Option<NativeAgentBoardView<'a>>,
    #[cfg(windows)]
    worktree: Option<&'a crate::terminal_session::state::worktree_launch::Launcher>,
    board_summary: Option<&'a AgentBoardSummary>,
    #[cfg(windows)]
    copy_mode: Option<crate::terminal_copy_mode::CopyModeView<'a>>,
    #[cfg(windows)]
    attention: Option<&'a super::agent_attention::AgentAttention>,
    #[cfg(windows)]
    reports: Option<&'a super::panel_reports::PanelReports>,
    #[cfg(windows)]
    graphics: Option<&'a super::pane_graphics::Driver>,
    #[cfg(windows)]
    graphics_surface: Option<crate::tui_graphics::Surface>,
    #[cfg(windows)]
    popup_failed: bool,
    #[cfg(windows)]
    command_notice: Option<NativeSessionNotice>,
    #[cfg(windows)]
    popup: Option<&'a super::independent_popup::Driver>,
}

impl<'a> NativeSessionView<'a> {
    #[cfg(windows)]
    pub(crate) fn selected_tab_ids(&self) -> Vec<orchbee_workbench_protocol::WorkbenchTabId> {
        self.state
            .as_ref()
            .map_or_else(Vec::new, StateSessionView::selected_tab_ids)
    }
    #[cfg(windows)]
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
    #[cfg(windows)]
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
                    && self.pane_projection_status(panel) == NativeProjectionStatus::FrameAvailable
            })
            .and_then(|panel| self.pane_projection(panel))
            .and_then(|projection| projection.frame())
            .and_then(|frame| frame.title.as_deref())
            .and_then(crate::terminal_title::stripped)
            .unwrap_or("");
        orchbee_workbench_protocol::WindowTitleContext::new(workspace, &label, manual, title)
    }

    #[cfg(windows)]
    pub(crate) fn panel_report(&self, panel: &super::NativePanel) -> super::PanelReportView<'_> {
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
            .map_or(0, StateSessionView::context_count)
    }
    #[cfg(windows)]
    pub(crate) fn navigation_site(&self) -> Option<&orchbee_workbench_protocol::SiteId> {
        self.state.as_ref().map(StateSessionView::navigation_site)
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
        #[cfg(windows)]
        if self.popup_failed {
            return Some(NativeSessionNotice::PopupUnavailable);
        }
        #[cfg(windows)]
        if self.command_notice.is_some() {
            return self.command_notice;
        }
        self.state.as_ref()?.notice().map(|notice| match notice {
            SessionNotice::LocalBusy => NativeSessionNotice::LocalBusy,
            SessionNotice::NoTabAtPosition(position) => {
                NativeSessionNotice::NoTabAtPosition(position)
            }
            SessionNotice::CreateNotCreated => NativeSessionNotice::CreateNotCreated,
            SessionNotice::DuplicatePending => NativeSessionNotice::DuplicatePending,
            SessionNotice::DuplicateNotStarted => NativeSessionNotice::DuplicateNotStarted,
            SessionNotice::DuplicateOutcomeUnknown => NativeSessionNotice::DuplicateOutcomeUnknown,
            SessionNotice::MoveNotMoved => NativeSessionNotice::MoveNotMoved,
            SessionNotice::MoveOrderConflict => NativeSessionNotice::MoveOrderConflict,
            SessionNotice::PanelNotApplied => NativeSessionNotice::PanelNotApplied,
            SessionNotice::RenameNotRenamed => NativeSessionNotice::RenameNotRenamed,
            SessionNotice::CloseRequestNotApplied => NativeSessionNotice::CloseRequestNotApplied,
            SessionNotice::WorkspaceCloseUnavailable => {
                NativeSessionNotice::WorkspaceCloseUnavailable
            }
            SessionNotice::ConfirmationCancelled => NativeSessionNotice::ConfirmationCancelled,
            SessionNotice::DraftCancelled => NativeSessionNotice::DraftCancelled,
            SessionNotice::ModalOwnsInput => NativeSessionNotice::ModalOwnsInput,
            SessionNotice::HistoryRecovering => NativeSessionNotice::HistoryRecovering,
            SessionNotice::HistoryUnavailable => NativeSessionNotice::HistoryUnavailable,
            SessionNotice::CopyModeUnavailable => NativeSessionNotice::CopyModeUnavailable,
            SessionNotice::CopyModeBusy => NativeSessionNotice::CopyModeBusy,
            SessionNotice::CopyModeInvalidated => NativeSessionNotice::CopyModeInvalidated,
            SessionNotice::CopyModeQueryFailed => NativeSessionNotice::CopyModeQueryFailed,
            SessionNotice::CopyModeNoMatch => NativeSessionNotice::CopyModeNoMatch,
            SessionNotice::CopyModeSearchLimit => NativeSessionNotice::CopyModeSearchLimit,
            SessionNotice::CopyModeNoSelection => NativeSessionNotice::CopyModeNoSelection,
            SessionNotice::SelectionEvicted => NativeSessionNotice::SelectionEvicted,
            SessionNotice::CopyFailed => NativeSessionNotice::CopyFailed,
            SessionNotice::CopyUnknown => NativeSessionNotice::CopyUnknown,
            SessionNotice::BellFailed => NativeSessionNotice::BellFailed,
            SessionNotice::BellUnknown => NativeSessionNotice::BellUnknown,
            #[cfg(windows)]
            SessionNotice::PositionChanged => NativeSessionNotice::PositionChanged,
            #[cfg(windows)]
            SessionNotice::PositionUnavailable => NativeSessionNotice::PositionUnavailable,
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

    /// The focus pane's projection — kept for single-pane consumers; paned
    /// rendering uses [`Self::pane_projection`] per leaf.
    pub(crate) fn projection(&self) -> Option<&'a TerminalProjection> {
        self.focus.and_then(|panel| self.pane_projection(panel))
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
            || self.pane_projection_status(panel) != NativeProjectionStatus::Empty
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
    ) -> NativeProjectionStatus {
        let Some(state) = self.state.as_ref() else {
            return NativeProjectionStatus::Empty;
        };
        let Some(record) = state.pane_by_panel(panel) else {
            return NativeProjectionStatus::Empty;
        };
        self.panes
            .iter()
            .find(|projection| projection.key == record.pane_ref())
            .map_or(NativeProjectionStatus::Empty, |projection| {
                projection.status
            })
    }

    /// One panel's retained-history surface, or `None` while its pane is
    /// Live. The returned window is already fenced to the pane's current
    /// generation and column geometry — the renderer only clips it to the
    /// leaf bounds.
    pub(crate) fn pane_viewport(
        &self,
        panel: &WorkbenchPanelId,
    ) -> Option<NativeViewportSurface<'_>> {
        let state = self.state.as_ref()?;
        let record = state.pane_by_panel(panel)?;
        let projection = self
            .panes
            .iter()
            .find(|projection| projection.key == record.pane_ref())?;
        let viewport = projection.viewport.as_ref()?;
        Some(match viewport.surface() {
            ViewportSurface::Window(window) => NativeViewportSurface::Window {
                window,
                position: viewport.position(),
            },
            ViewportSurface::Entering => NativeViewportSurface::Entering,
            ViewportSurface::AwaitingWindow => NativeViewportSurface::AwaitingWindow,
            ViewportSurface::Recovering => NativeViewportSurface::Recovering,
        })
    }

    /// Capture the exact canonical member and terminal row surface this view
    /// would paint for one panel. Loading indicators and unbound panes expose
    /// no link source.
    #[cfg(windows)]
    pub(crate) fn painted_content_source(
        &self,
        panel_id: &WorkbenchPanelId,
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
        let host_terminal =
            orchbee_client_server_protocol::HostTerminalId::parse(projection.host_terminal_id()?)
                .ok()?;
        let (screen, rows) = match self.pane_viewport(panel_id) {
            Some(NativeViewportSurface::Window { window, .. }) => (
                TerminalScreen::Primary,
                PaintedRows {
                    attachment: window.route.attachment_id,
                    host_epoch: window.route.host_epoch,
                    sequence: window.route.sequence,
                    generation: window.route.generation,
                    revision: window.route.revision,
                    columns: window.route.columns,
                    total_rows: window.route.total_rows,
                    start_row: window.route.start_row,
                    row_count: window.route.row_count,
                    history: true,
                },
            ),
            Some(
                NativeViewportSurface::Entering
                | NativeViewportSurface::AwaitingWindow
                | NativeViewportSurface::Recovering,
            ) => return None,
            None => {
                let frame = projection.frame()?;
                (TerminalScreen::from_frame(frame), PaintedRows::live(frame)?)
            }
        };
        Some(ContentSource {
            pane,
            member: SnapshotMember {
                site: state.site_id().clone(),
                context: state.terminal_context_id().clone(),
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

    #[cfg(windows)]
    pub(crate) fn painted_link_source(
        &self,
        panel_id: &WorkbenchPanelId,
    ) -> Option<crate::terminal_link_activation::PaintedLinkSource> {
        self.painted_content_source(panel_id)
    }

    /// The panel identity of the focus pane — the leaf that takes input,
    /// resize and the focused-frame chrome.
    pub(crate) const fn focus_panel_id(&self) -> Option<&'a WorkbenchPanelId> {
        self.focus
    }
    #[cfg(windows)]
    pub(crate) fn focused_member(&self) -> Option<super::NativeMemberTarget> {
        if self.retirement.is_some() {
            return None;
        }
        let tab = self.tab(self.active_position()?.checked_sub(1)?)?;
        let panel = (0..tab.panel_count())
            .filter_map(|i| tab.panel(i))
            .find(|p| Some(p.panel_id()) == self.focus_panel_id())?;
        Some(super::NativeMemberTarget {
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: self.selected_presentation(panel).presentation_id().clone(),
        })
    }

    /// The active Tab renders zoomed: the focused leaf draws across the
    /// whole pane content area while hidden leaves stay attached but
    /// undrawn. Volatile Client-local session state.
    pub(crate) fn zoom_active(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(StateSessionView::zoom_active)
    }

    pub(crate) const fn retirement(&self) -> Option<NativeDisplayRetirement> {
        self.retirement
    }

    /// The Agent board's read handle — `Some` only while the board is
    /// open, so a closed board never paints and never claims hits.
    pub(crate) const fn agent_board(&self) -> Option<&NativeAgentBoardView<'a>> {
        self.board.as_ref()
    }

    /// The sidebar's summary of the last completed Agent read — `None`
    /// when no read has completed, never an implied zero.
    pub(crate) const fn agent_board_summary(&self) -> Option<AgentBoardSummary> {
        self.board_summary.copied()
    }

    #[cfg(windows)]
    pub(crate) const fn copy_mode(&self) -> Option<crate::terminal_copy_mode::CopyModeView<'a>> {
        self.copy_mode
    }
}

#[cfg(test)]
impl<'a> NativeSessionView<'a> {
    /// Empty, unobstructed Native surface for consumers whose test owns no session state.
    #[cfg(windows)]
    pub(super) fn notification_test_view() -> Self {
        Self {
            worktree: None,
            #[cfg(windows)]
            menu: None,
            #[cfg(windows)]
            settings: None,
            state: None,
            panes: &[],
            retained_frames: None,
            focus: None,
            projection_status: NativeProjectionStatus::Empty,
            retirement: None,
            board: None,
            board_summary: None,
            copy_mode: None,
            attention: None,
            reports: None,
            graphics: None,
            graphics_surface: None,
            popup_failed: false,
            command_notice: None,
            popup: None,
        }
    }

    /// Test-only view over a bare session state: the caller supplies the
    /// live projections and the focus draw status the paint pass would
    /// report — the exact shape `pane_geometries`, `active_pane_area`, and
    /// the renderer consume.
    pub(super) fn test_view(
        state: &'a NativeSessionState,
        panes: &'a [KeyedProjection],
        projection_status: NativeProjectionStatus,
    ) -> Self {
        Self {
            #[cfg(windows)]
            worktree: None,
            #[cfg(windows)]
            menu: None,
            #[cfg(windows)]
            settings: None,
            state: Some(state.view()),
            panes,
            retained_frames: None,
            focus: state.focus_panel_id(),
            projection_status,
            retirement: None,
            board: None,
            board_summary: None,
            #[cfg(windows)]
            copy_mode: None,
            #[cfg(windows)]
            attention: None,
            #[cfg(windows)]
            reports: None,
            #[cfg(windows)]
            graphics: None,
            #[cfg(windows)]
            graphics_surface: None,
            #[cfg(windows)]
            popup_failed: false,
            #[cfg(windows)]
            command_notice: None,
            #[cfg(windows)]
            popup: None,
        }
    }
}

/// One pane's installed projection plus its draw status, keyed by the exact
/// incarnation the `Attached` event settled.
pub(crate) struct KeyedProjection {
    key: PaneRef,
    value: TerminalProjection,
    status: NativeProjectionStatus,
    /// The retained-history core. `None` is exactly the Live mode — a pane
    /// carries no viewport state at all until its `Enter` step is admitted.
    viewport: Option<TerminalViewport>,
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
    /// Test-only record for [`NativeSessionView::test_view`]: a live
    /// projection bound to an exact pane incarnation.
    pub(super) fn for_test(
        key: PaneRef,
        value: TerminalProjection,
        status: NativeProjectionStatus,
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

pub(super) struct NativeUi {
    #[cfg(windows)]
    settings: settings::Coordinator,
    #[cfg(windows)]
    menus: action_menus::Coordinator,
    mailbox: Option<Arc<CommandMailbox>>,
    clock: LocalOperationClock,
    outer_focused: Option<bool>,
    state: Option<NativeSessionState>,
    board: super::agent_board::AgentBoard,
    #[cfg(windows)]
    worktrees: worktree_launch::Coordinator,
    #[cfg(windows)]
    attention: super::agent_attention::AgentAttention,
    #[cfg(windows)]
    reports: super::panel_reports::PanelReports,
    #[cfg(windows)]
    graphics: super::pane_graphics::Driver,
    #[cfg(windows)]
    overlays: super::plugin_overlays::Driver,
    #[cfg(windows)]
    links: crate::terminal_link_activation::Driver,
    #[cfg(windows)]
    copy_mode: copy_mode::Coordinator,
    #[cfg(windows)]
    popup: super::independent_popup::Driver,
    #[cfg(windows)]
    command_notice: Option<(NativeSessionNotice, std::time::Instant)>,
    /// `presentation_id → workbench placement` over the freshest observed
    /// canonical snapshot — a pure function of that snapshot, rebuilt
    /// whole on every snapshot-bearing event.
    agent_index: super::agent_board::PresentationIndex,
    projections: Vec<KeyedProjection>,
    retained_frames: retained_frames::RetainedFrames,
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
    retirement: Option<NativeDisplayRetirement>,
    #[cfg(any(target_os = "linux", windows))]
    retired_mailbox_seed: Option<super::mailbox::RetiredMailboxSeed>,
    #[cfg(any(target_os = "linux", windows))]
    retirement_poll_serial: Option<u64>,
    #[cfg(any(target_os = "linux", windows))]
    retired_peer: Option<super::state::retired_membership::RetiredMembershipOrigin>,
    #[cfg(any(target_os = "linux", windows))]
    retired_peer_attempt: Option<retired_membership::RetiredMembershipAttempt>,
}

#[cfg(test)]
mod agent_board_tests;
#[cfg(test)]
mod context_selection_tests;
#[cfg(test)]
mod create_membership_tests;
#[cfg(test)]
mod location_labels_tests;
#[cfg(test)]
pub(in crate::terminal_session) mod membership_tests;
#[cfg(test)]
pub(in crate::terminal_session) mod move_membership_tests;
#[cfg(test)]
mod panel_input_tests;
#[cfg(all(test, windows))]
mod panel_reports_tests;
#[cfg(test)]
pub(in crate::terminal_session) mod publication_hint_tests;
#[cfg(test)]
mod rename_membership_tests;
#[cfg(all(test, any(target_os = "linux", windows)))]
mod retired_membership_tests;
mod scrollbar;
#[cfg(test)]
mod startup_queue_tests;
#[cfg(test)]
mod u7_probe;
#[cfg(test)]
mod viewport_tests;

mod client_presentation;
mod client_viewport;
#[cfg(test)]
mod control_takeover_tests;
#[cfg(windows)]
mod independent_popup;
#[cfg(windows)]
mod link_activation;
#[cfg(all(test, windows))]
mod quick_workspace_tests;
#[cfg(test)]
pub(in crate::terminal_session) mod workspace_close_tests;
#[cfg(test)]
mod workspace_setup_tests;
#[cfg(all(test, windows))]
mod worktree_launch_tests;

impl NativeUi {
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
        let close = self
            .state
            .as_ref()
            .and_then(NativeSessionState::close_set_deadline);
        let summary = self
            .state
            .as_ref()
            .and_then(NativeSessionState::close_set_summary_deadline);
        ordinary
            .into_iter()
            .chain(close)
            .chain(summary)
            .min()
            .map(|at| at.saturating_duration_since(self.clock.now()))
    }

    #[cfg(windows)]
    pub(super) fn host_focus(&self) -> Option<bool> {
        self.outer_focused
    }
    #[cfg(windows)]
    pub(super) fn outer_title_focused(&self) -> bool {
        self.outer_focused.unwrap_or(false)
    }
    #[cfg(windows)]
    pub(super) fn configuration_clock(&self) -> LocalOperationClock {
        self.clock.clone()
    }
    pub(super) fn new(mailbox: Arc<CommandMailbox>, clock: LocalOperationClock) -> Self {
        Self {
            mailbox: Some(mailbox),
            board: super::agent_board::AgentBoard::new(clock.clone()),
            #[cfg(windows)]
            worktrees: worktree_launch::Coordinator::default(),
            #[cfg(windows)]
            menus: action_menus::Coordinator::default(),
            #[cfg(windows)]
            settings: settings::Coordinator::default(),
            #[cfg(windows)]
            attention: super::agent_attention::AgentAttention::new(clock.clone()),
            #[cfg(windows)]
            reports: super::panel_reports::PanelReports::new(clock.clone()),
            #[cfg(windows)]
            graphics: super::pane_graphics::Driver::new(clock.clone()),
            #[cfg(windows)]
            overlays: super::plugin_overlays::Driver::new(clock.clone()),
            #[cfg(windows)]
            links: crate::terminal_link_activation::Driver::new(clock.clone()),
            #[cfg(windows)]
            copy_mode: copy_mode::Coordinator::new(clock.clone()),
            #[cfg(windows)]
            popup: super::independent_popup::Driver::new(clock.clone()),
            #[cfg(windows)]
            command_notice: None,
            agent_index: super::agent_board::PresentationIndex::default(),
            clock,
            state: None,
            outer_focused: None,
            projections: Vec::new(),
            retained_frames: retained_frames::RetainedFrames::default(),
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
            #[cfg(any(target_os = "linux", windows))]
            retired_mailbox_seed: None,
            #[cfg(any(target_os = "linux", windows))]
            retirement_poll_serial: None,
            #[cfg(any(target_os = "linux", windows))]
            retired_peer: None,
            #[cfg(any(target_os = "linux", windows))]
            retired_peer_attempt: None,
        }
    }

    pub(super) fn view(&self) -> NativeSessionView<'_> {
        let state = self.state.as_ref();
        let focus = state.and_then(NativeSessionState::focus_panel_id);
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
            .map_or(NativeProjectionStatus::Empty, |projection| {
                projection.status
            });
        NativeSessionView {
            state: state.map(NativeSessionState::view),
            panes: &self.projections,
            retained_frames: state.map(|state| self.retained_frames.view(state)),
            focus,
            projection_status: focus_status,
            retirement: self.retirement,
            board: self.agent_board_view(),
            board_summary: self.board.summary(),
            #[cfg(windows)]
            copy_mode: self.copy_mode_view(),
            #[cfg(windows)]
            attention: Some(&self.attention),
            #[cfg(windows)]
            reports: state
                .filter(|s| self.reports.matches(s))
                .map(|_| &self.reports),
            #[cfg(windows)]
            graphics: state
                .filter(|_| self.retirement.is_none())
                .map(|_| &self.graphics),
            #[cfg(windows)]
            graphics_surface: None,
            #[cfg(windows)]
            popup_failed: self.popup.failed(),
            #[cfg(windows)]
            command_notice: self
                .settings
                .notice(self.clock.now())
                .map(NativeSessionNotice::Settings)
                .or_else(|| self.links.notice().map(link_notice))
                .or_else(|| {
                    self.command_notice
                        .filter(|(_, until)| self.clock.now() < *until)
                        .map(|(notice, _)| notice)
                }),
            #[cfg(windows)]
            popup: Some(&self.popup),
            #[cfg(windows)]
            worktree: Some(&self.worktrees.launcher),
            #[cfg(windows)]
            menu: Some(&self.menus.menu),
            #[cfg(windows)]
            settings: Some(&self.settings),
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
        if self.view().projection_status() == NativeProjectionStatus::Blocked {
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
        self.retirement_failure.is_none()
            && (!self.pending_draws.is_empty()
                || self.state.as_ref().is_some_and(|state| {
                    matches!(
                        state.phase_kind(),
                        SessionPhaseKind::Empty | SessionPhaseKind::SelectedWithoutAttachment
                    )
                }))
    }

    pub(super) const fn retirement_requested(&self) -> bool {
        self.retirement_failure.is_some() && !self.retirement_finalized
    }

    pub(super) fn release_mailbox(&mut self) {
        #[cfg(any(target_os = "linux", windows))]
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
        self.retained_frames.clear();
        self.clear_projections();
        self.retirement_finalized = true;
        self.retirement = Some(if failure == CarrierFailure::OutcomeUnknown {
            NativeDisplayRetirement::OutcomeUnknown
        } else {
            NativeDisplayRetirement::Unavailable
        });
        accepted
    }

    pub(super) fn tick(&mut self) -> bool {
        if let Some(state) = &self.state {
            self.retained_frames.prune(state);
        }
        #[cfg(windows)]
        let command_notice_changed = if self
            .command_notice
            .is_some_and(|(_, until)| self.clock.now() >= until)
        {
            self.command_notice = None;
            true
        } else {
            false
        };
        #[cfg(not(windows))]
        let command_notice_changed = false;
        let effect_changed = self.settle_effect_delivery();
        if self.retirement_failure.is_some() {
            return command_notice_changed;
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
        // An armed membership-recovery attempt owns the mailbox until its
        // candidate installs or the attempt settles: suppress the ordinary
        // post-event scheduler block so the queue carries the Refresh alone.
        if self.retirement_failure.is_none() && !self.retired_membership_attempt_active() {
            changed |= self
                .state
                .as_mut()
                .is_some_and(|state| state.settle_stale_close_confirmations());
            self.reconcile_ordinary_poll(&mailbox);
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
            // The board lane and the jump stage machine live on the same
            // fair tick as every other scheduled admission.
            changed |= self.drive_agent_board();
            #[cfg(windows)]
            {
                if let Some(state) = self.state.as_mut()
                    && self.overlays.tick(state)
                {
                    state.note_publication_hint();
                    changed = true;
                }
                changed |= self.drive_worktree();
                changed |= self.drive_action_menu();
                changed |= self.drive_settings();
                changed |= self.links.tick();
                changed |= self.popup.tick();
                changed |= self.graphics.tick(self.state.as_ref());
                changed |= self.attention.tick();
                changed |= self.reports.tick(
                    self.state
                        .as_ref()
                        .and_then(super::panel_reports::Scope::capture),
                );
            }
            // Viewport deadline episodes and staged steps advance on the
            // same fair tick as the ordinary schedulers.
            changed |= self.tick_viewports(&mailbox);
            self.admit_selections(&mailbox);
            #[cfg(windows)]
            {
                changed |= self.tick_copy_mode();
            }
        }
        acknowledged
            || command_notice_changed
            || changed
            || effect_changed
            || self.effects.has_pending()
            || !self.pending_draws.is_empty()
    }

    pub(super) fn after_draw(&mut self) {
        // The shell calls this only after Backend::flush closed the frame.
        if let Some(state) = &self.state {
            self.retained_frames.after_draw(state, &self.projections);
        }
        self.dispatch_effect_after_draw();
        if self.pending_draws.is_empty() {
            return;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return;
        };
        // One completed draw settles every staged frame: each pane's poll
        // keeps its own causal slot, so draining the queue deposits them all.
        let pending_draws = std::mem::take(&mut self.pending_draws);
        for pending in pending_draws {
            // The settlement claims this draw's proof: a frame that was never
            // staged into a live, non-departing leaf settles as `NoFeedback` —
            // the poll-time `FrameApplied` is never forwarded for a frame the
            // screen never showed.
            let settlement = match pending.frame {
                Some(frame) => {
                    let staged = self.state.as_mut().is_some_and(|state| {
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
            self.outer_focused = focused;
            #[cfg(windows)]
            self.attention.focus_changed();
        }
    }

    pub(super) fn observe_geometry(&mut self, geometry: Geometry) {
        #[cfg(windows)]
        self.observe_action_menu_geometry(geometry);
        if self
            .state
            .as_ref()
            .is_some_and(|state| state.latest_geometry() != geometry)
        {
            self.invalidate_close_display();
            self.invalidate_rename_display();
            self.invalidate_setup_display();
            #[cfg(windows)]
            {
                self.after_worktree_draw(None);
                self.fail_action_menu();
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
            // A non-Session close carrier — status-only recovery, or the
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
            if state.phase_kind() == SessionPhaseKind::Starting {
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
    /// caller: its mailbox carries the recovery Refresh alone. A non-Session
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
        if state.phase_kind() == SessionPhaseKind::Starting || state.close_status_only() {
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
    /// candidate installs or the attempt settles. The attempt record exists
    /// only on Linux; other platforms have no recovery path.
    fn retired_membership_attempt_active(&self) -> bool {
        #[cfg(any(target_os = "linux", windows))]
        {
            self.retired_peer_attempt.is_some()
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            false
        }
    }

    pub(super) fn forward(&mut self, input: &[u8]) -> bool {
        if self.retirement_failure.is_some() {
            return false;
        }
        let Some(state) = self.state.as_ref() else {
            return false;
        };
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
                .find(|projection| projection.key == desired)
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
    fn enter_history(&mut self) -> SessionActionDisposition {
        self.enter_history_rows(None)
    }

    /// Keyboard entry uses a page; pointer entry supplies its line count.
    /// Both share the same admission, incarnation and demand ownership.
    fn enter_history_rows(&mut self, rows_before: Option<u32>) -> SessionActionDisposition {
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return SessionActionDisposition::KeepNavigator;
        };
        let Some(state) = self.state.as_ref() else {
            return SessionActionDisposition::KeepNavigator;
        };
        if state.ready().is_none() {
            return SessionActionDisposition::KeepNavigator;
        }
        let Some(key) = state.focus_pane_ref() else {
            return SessionActionDisposition::KeepNavigator;
        };
        if state.close_suppresses_attachment(key.pane().anchor()) {
            return SessionActionDisposition::KeepNavigator;
        }
        let Some(index) = self.projections.iter().position(|p| p.key == key) else {
            return SessionActionDisposition::KeepNavigator;
        };
        if self.viewport_admission_blocked(&key) {
            return SessionActionDisposition::KeepNavigator;
        }
        if self.projections[index].viewport.is_some() {
            return SessionActionDisposition::KeepNavigator;
        }
        // History needs a live-tail anchor plus retained rows: a pane that
        // never painted has no scrollback contract, an alternate screen has
        // no retained history at all, and zero scrollback or zero readable
        // rows means nothing exists to view. Every refusal is an honest
        // notice — never a silent drop, never a fabricated Enter command.
        let Some(frame) = self.projections[index].value.frame() else {
            return self.history_unavailable(&mailbox);
        };
        if frame.scrollback_rows == 0 || frame.alternate_screen || frame.rows.is_empty() {
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
        SessionActionDisposition::KeepNavigator
    }

    /// The Enter request cannot honestly proceed: publish
    /// `HistoryUnavailable` and send nothing on the wire.
    fn history_unavailable(&mut self, mailbox: &Arc<CommandMailbox>) -> SessionActionDisposition {
        if let Some(state) = self.state.as_mut() {
            mailbox.apply_local_for(state, |state| {
                state.set_notice(SessionNotice::HistoryUnavailable);
            });
        }
        SessionActionDisposition::KeepNavigator
    }

    /// Whether the focus pane's non-Live core consumes this key locally.
    /// The five history keys scroll or exit; every other key reports
    /// `false` so the ordinary byte path forwards (and exits history
    /// first). Live panes always report `false` — their passthrough is
    /// untouched.
    pub(super) fn history_key(&mut self, event: &crate::client_view::KeyEvent) -> bool {
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return false;
        };
        let Some(key) = self.state.as_ref().and_then(|state| state.focus_pane_ref()) else {
            return false;
        };
        let Some(index) = self.projections.iter().position(|p| p.key == key) else {
            return false;
        };
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
    /// core by three rows. Recheck focus identity at this mutation boundary;
    /// the pointer layer owns content hit testing and Shift exclusion.
    pub(super) fn history_wheel(&mut self, key: &PaneRef, up: bool) -> bool {
        if self
            .state
            .as_ref()
            .and_then(|state| state.focus_pane_ref())
            .as_ref()
            != Some(key)
        {
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
                        && frame.scrollback_rows > 0
                        && !frame.rows.is_empty()
                });
            if !eligible {
                return false;
            }
            self.enter_history_rows(Some(3));
            return self
                .projections
                .iter()
                .any(|projection| &projection.key == key && projection.viewport.is_some());
        }
        let Some(viewport) = self.projections[index].viewport.as_mut() else {
            return false;
        };
        let effect = viewport.nav(ViewportNav::Lines(if up { 3 } else { -3 }));
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
                self.projections
                    .iter()
                    .any(|projection| projection.key == key && projection.viewport.is_some())
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
            if let Some(projection) = self.projections.iter_mut().find(|p| &p.key == key) {
                projection.viewport = None;
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
                || {
                    #[cfg(windows)]
                    {
                        self.worktree_input_context().is_some() || self.action_menu_active()
                    }
                    #[cfg(not(windows))]
                    {
                        false
                    }
                }
                || view.close_confirmation().is_some()
                || view.workspace_close_confirmation().is_some(),
            ..PointerProbe::default()
        };
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
                    == NativeProjectionStatus::FrameAvailable,
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
        #[cfg(any(not(windows), test))]
        {
            probe.contexts = (0..view.context_count())
                .filter_map(|index| view.context_at(index))
                .map(|context| context.location().clone())
                .collect();
        }
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

    pub(super) fn apply_client_action(&mut self, action: ClientAction) -> SessionActionDisposition {
        #[cfg(windows)]
        if let Some(state) = self.state.as_mut() {
            // New intent wins even when its admission is refused.
            state.cancel_project_quick_entry();
        }
        #[cfg(windows)]
        self.cancel_action_menu();
        if self.retirement_failure.is_some() {
            return SessionActionDisposition::KeepNavigator;
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
        match action {
            #[cfg(windows)]
            ClientAction::OpenActionMenu => SessionActionDisposition::KeepNavigator,
            #[cfg(windows)]
            ClientAction::OpenSettings => self.request_settings(),
            #[cfg(windows)]
            ClientAction::OpenMachines
            | ClientAction::ConfiguredCommand(_)
            | ClientAction::PreviousAgent
            | ClientAction::NextAgent
            | ClientAction::FocusAgent(_) => SessionActionDisposition::KeepNavigator,
            #[cfg(windows)]
            ClientAction::RequestWorktreeCreate => self.request_worktree(WorktreeMode::Create),
            #[cfg(windows)]
            ClientAction::RequestWorktreeOpen => self.request_worktree(WorktreeMode::Open),
            ClientAction::RequestQuickWorkspace => {
                self.request_quick_workspace();
                SessionActionDisposition::KeepNavigator
            }
            ClientAction::RequestProjectQuickWorkspace(request) => {
                if !self.request_quick_workspace_for(Some(*request))
                    && let Some(state) = self.state.as_mut()
                {
                    state.set_notice(super::state::SessionNotice::DraftCancelled);
                }
                SessionActionDisposition::KeepNavigator
            }
            ClientAction::RequestTargetedProjectRename(request) => {
                if self.request_project_rename(request).is_none()
                    && let Some(state) = self.state.as_mut()
                {
                    state.set_notice(super::state::SessionNotice::DraftCancelled);
                }
                SessionActionDisposition::KeepNavigator
            }
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
            #[cfg(windows)]
            ClientAction::TabMenu { action, tab_id } => {
                self.apply_tab_menu(action, tab_id.as_ref())
            }
            #[cfg(windows)]
            ClientAction::ToggleTabGroup(group) => self.toggle_tab_group(&group),
            ClientAction::RequestRenameTab => {
                if self.request_rename_draft().is_some() {
                    SessionActionDisposition::DismissNavigator
                } else {
                    SessionActionDisposition::KeepNavigator
                }
            }
            ClientAction::RequestCloseTab => self.request_close_confirmation(),
            ClientAction::RequestWorkspaceClose => self.request_workspace_close(),
            ClientAction::RequestControlTakeover => self.request_takeover(),
            #[cfg(windows)]
            ClientAction::EnterResizeMode => {
                let _consumed = self.history_key(&crate::client_view::KeyEvent::press(
                    crate::client_view::KeyCode::Esc,
                    crate::client_view::Modifiers::NONE,
                ));
                SessionActionDisposition::KeepNavigator
            }
            #[cfg(windows)]
            ClientAction::ResizePane { .. } | ClientAction::ExitResizeMode => {
                SessionActionDisposition::KeepNavigator
            }
            ClientAction::ConfirmClose => self.confirm_close(),
            ClientAction::CancelClose => self.cancel_close(),
            ClientAction::CreateTab => self.create_tab(),
            ClientAction::MovePreviousTab => self.move_tab(WorkbenchTabMoveDirection::Previous),
            ClientAction::MoveNextTab => self.move_tab(WorkbenchTabMoveDirection::Next),
            ClientAction::SelectPreviousContext => self.select_context(false),
            ClientAction::SelectNextContext => self.select_context(true),
            #[cfg(any(not(windows), test))]
            ClientAction::SelectContextAt(index) => self.select_context_at(index),
            ClientAction::SelectNextTab
            | ClientAction::SelectPreviousTab
            | ClientAction::SelectTab(_) => self.select_tab(action),
            ClientAction::EnterHistory => self.enter_history(),
            #[cfg(windows)]
            ClientAction::EnterCopyMode => self.enter_copy_mode(),
            ClientAction::PreviousPresentation => self.cycle_presentation(false),
            ClientAction::NextPresentation => self.cycle_presentation(true),
            ClientAction::SelectPresentation {
                panel,
                presentation,
            } => self.select_presentation_in_active_tab(&panel, &presentation),
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
            #[cfg(windows)]
            ClientAction::FocusBusinessSidebar => SessionActionDisposition::KeepNavigator,
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
        self.create_tab_placed(None)
    }

    fn create_tab_placed(
        &mut self,
        placement: Option<orchbee_workbench_protocol::TabCreationPlacement>,
    ) -> SessionActionDisposition {
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
            |state, generation, request_id| {
                build_create(state, generation, request_id, deadlines, placement)
            },
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
        #[cfg(windows)]
        if let Some(state) = &mut self.state {
            state.clear_tab_selection();
        }
        if self.select_tab_generation(action).is_some() {
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }

    fn select_tab_generation(&mut self, action: ClientAction) -> Option<u64> {
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

    /// Pointer-issued context selection: the row index was resolved from
    /// the painted navigator, so the same refresh machinery targets it
    /// directly instead of stepping next/previous.
    fn select_context_at(&mut self, index: usize) -> SessionActionDisposition {
        self.select_context_at_generation(index);
        SessionActionDisposition::KeepNavigator
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
    fn move_panel_focus_to(&mut self, panel: &WorkbenchPanelId) -> SessionActionDisposition {
        self.move_panel_focus_admitted(panel)
            .unwrap_or(SessionActionDisposition::KeepNavigator)
    }

    fn move_panel_focus_admitted(
        &mut self,
        panel: &WorkbenchPanelId,
    ) -> Option<SessionActionDisposition> {
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
            return Some(SessionActionDisposition::LayoutChanged);
        }
        Some(SessionActionDisposition::KeepNavigator)
    }

    /// `Ctrl+B z`: the volatile Client-local zoom toggle. The mailbox lock
    /// serializes flag minting against membership installs so a flag can
    /// never be minted on a Tab an in-flight install is shrinking to one
    /// leaf — the same TOCTOU the local confirmations close.
    fn toggle_zoom(&mut self) -> SessionActionDisposition {
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        if !mailbox.apply_local_for(state, NativeSessionState::toggle_zoom) {
            return SessionActionDisposition::KeepNavigator;
        }
        SessionActionDisposition::LayoutChanged
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
        if self.panel_close_confirmation_armed() {
            return SessionActionDisposition::KeepNavigator;
        }
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

    /// A directional focus move is a pure local transition: the neighbour
    /// pane's viewer is already live under the same anchor, so no
    /// detach/attach round trip is minted. The mailbox's desired key is
    /// republished so focus-scoped admission (input/resize) follows the move.
    fn move_panel_focus(&mut self, direction: PanelDirection) -> SessionActionDisposition {
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let Some(pane) = state.plan_panel_focus_local(direction) else {
            return SessionActionDisposition::KeepNavigator;
        };
        if !state.move_panel_focus_local(pane.clone()) {
            self.retire(CarrierFailure::Protocol);
            return SessionActionDisposition::KeepNavigator;
        }
        // Under zoom the drawn set follows the focus: the pointer map and
        // the observed geometry are stale the moment the move lands.
        let zoomed = state.zoom_active();
        if let Err(closed) = mailbox.publish_desired(pane) {
            self.retire(closed.failure());
        }
        if zoomed {
            return SessionActionDisposition::LayoutChanged;
        }
        SessionActionDisposition::KeepNavigator
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
                    let (state, target) = NativeSessionState::bootstrap_at(
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
            #[cfg(windows)]
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
                #[cfg(windows)]
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
                    state.set_notice(notice);
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
                            status: NativeProjectionStatus::Empty,
                            viewport: None,
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
        #[cfg(any(target_os = "linux", windows))]
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
            _ => None,
        };
        // Projection and append-only validation are bounded but intentionally run outside the
        // mailbox mutex. Only the prepared value is committed in the exact reply transaction.
        let prepared = state.prepare_refresh(generation, outcome);
        #[cfg(windows)]
        let prepared = self.fence_worktree_context_candidate(
            prepared,
            index_snapshot.as_ref(),
            generation,
            state.cache().revision(),
        );
        #[cfg(windows)]
        let prepared =
            self.fence_menu_context_candidate(prepared, generation, state.cache().revision());
        #[cfg(windows)]
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
                #[cfg(windows)]
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
                    #[cfg(windows)]
                    self.observe_worktree_entry_snapshot(snapshot);
                    self.agent_index = super::agent_board::PresentationIndex::rebuild(snapshot);
                    #[cfg(windows)]
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
                NativeProjectionStatus::FrameAvailable,
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
                    NativeProjectionStatus::WaitingForFullFrame,
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
        self.projections[index].status = status;
        if matches!(
            status,
            NativeProjectionStatus::Blocked | NativeProjectionStatus::WaitingForFullFrame
        ) && let Some(state) = self.state.as_ref()
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

    /// Schedules the next ordinary poll for the next pane in rotation. One
    /// ordinary poll stays in flight at a time — the single causal cell is
    /// preserved — while the cursor walks the projected pane set so every
    /// pane's viewer advances without starving the focus.
    fn schedule_poll(&mut self, mailbox: &Arc<CommandMailbox>) {
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
            && !(state.phase_kind() == SessionPhaseKind::SelectedWithoutAttachment
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
        #[cfg(windows)]
        self.copy_mode.close();
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
        #[cfg(windows)]
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
        #[cfg(windows)]
        {
            self.cancel_copy_mode();
            self.fail_action_menu();
        }
        self.effects.close();
        self.terminal_effect.take();
        self.clipboard.close();
        self.settle_effect_delivery();
        #[cfg(any(target_os = "linux", windows))]
        if self.retirement_failure.is_none() {
            self.retirement_poll_serial = self.ordinary_poll_serial;
        }
        self.board.close();
        #[cfg(windows)]
        self.reports.close();
        #[cfg(windows)]
        self.graphics.close();
        #[cfg(windows)]
        self.overlays.close();
        #[cfg(windows)]
        self.links.close();
        #[cfg(windows)]
        self.popup.shutdown();
        if let Some(state) = &mut self.state {
            state.close_carrier_retired();
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

const fn create_precommit_notice(outcome: &CreatePrecommitOutcome) -> SessionNotice {
    match outcome {
        CreatePrecommitOutcome::NotSubmitted
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Rejected { .. })
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Denied)
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Unavailable { .. })
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::ProfileUnavailable)
        | CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::ProfileAmbiguous {
            ..
        }) => SessionNotice::CreateNotCreated,
    }
}

fn build_create(
    state: &mut NativeSessionState,
    generation: u64,
    request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    deadlines: OperationDeadlines,
    placement: Option<orchbee_workbench_protocol::TabCreationPlacement>,
) -> Result<CreateMarker, ActionBuildError> {
    if !state.invariant_holds() {
        return Err(ActionBuildError::Protocol);
    }
    orchbee_workbench_protocol::validate_tab_creation(
        state.cache().canonical_context(),
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
        state.cache().site_id().clone(),
        state.cache().revision(),
        state.cache().terminal_context_id().clone(),
        label.clone(),
        geometry,
        deadlines,
    )
    .with_placement(placement.clone())
    .with_origin(if state.phase_kind() == SessionPhaseKind::Empty {
        super::state::CreateOriginKind::Empty
    } else {
        super::state::CreateOriginKind::Ready
    });
    if automatic_label {
        operation = operation.with_automatic_label();
    }
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id,
        site_id: state.cache().site_id().clone(),
        expected_revision: state.cache().revision(),
        action: WorkbenchAction::TabCreate {
            placement,
            automatic_label,
            terminal_context_id: state.cache().terminal_context_id().clone(),
            label,
            geometry: workbench_geometry,
            launch: None,
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

/// `build_switch` carrying the stable panel identity the Agent board's
/// jump wants focused after the tab switch lands — the same admission
/// checks, the same `begin_switch`, only the selected pane differs: it
/// resolves `panel_id` to its current index instead of `panel(0)`.
fn build_switch_for_panel(
    state: &mut NativeSessionState,
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

#[cfg(all(test, windows))]
mod window_title_tests;
