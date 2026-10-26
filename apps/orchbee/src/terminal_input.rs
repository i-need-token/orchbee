//! Defines the input port contract between decoded host events, Client capture owners, and
//! terminal forwarding, with explicit unavailable outcomes. It supplies routing vocabulary
//! without owning host reads, Server effects, or canonical Workbench state.

use crate::client_view::{ClientAction, ClientActionDisposition, ClientKey, ClientView};
use crate::tui_composition::{CaptureClaims, WorkbenchCaptureOwner};
use crate::workbench_client::{
    CaptureContext, RenameDraftToken, RenameEdit, SetupEdit, SetupToken,
};
use orchbee_client_server_protocol::Geometry;
use orchbee_terminal_protocol::{MouseFormat, MouseTracking};
use orchbee_workbench_protocol::WorkbenchTabId;

use crate::workbench_client::{WorktreeEdit, WorktreeToken};

mod decoder;
pub(crate) use decoder::LegacyDecoder;
pub(crate) use decoder::{DecodedItem, DecodedKind};
mod dispatcher;
mod modal;
mod mouse;
mod utf8;
pub(crate) use dispatcher::InputDispatcher;
pub(crate) use mouse::{
    MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent, PanePointerRoute,
    encode_cursor_arrows, encode_pane_report, route_pane_pointer,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalInputUnavailable;

/// Metadata replies produced by the outer terminal. These are consumed before
/// any keyboard, modal-editor or pane-input owner can observe their bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostReply {
    Graphics(crate::terminal_graphics::Reply),
    Pointer(crate::terminal_pointer_admission::HostReply),
    Appearance(crate::terminal_host_appearance::ColorReply),
}

/// The complete outer-terminal pointer reporting state requested by input.
/// The shell publishes the matching dispatcher state only after the lifecycle
/// write succeeds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PointerCaptureRequest {
    #[default]
    Release,
    ButtonOnly,
    AnyMotion,
    /// A capturing menu requests idle motion while retaining the button-only fallback.
    MenuMotion,
}

/// One terminal-control write the dispatcher or surface asks the shell to
/// perform. The lifecycle layer owns the actual bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModeRequest {
    /// One pane bell, emitted after the completed frame has flushed.
    Bell,
    /// Bounded metadata queries on the already-admitted host terminal.
    GraphicsQuery(crate::terminal_graphics::Query),
    /// The endpoint's sole any-motion capability query, after ButtonOnly is active.
    PointerAdmissionQuery(crate::terminal_pointer_admission::Query),
    /// One bounded OSC 10/11 host-default color query at terminal admission.
    AppearanceQuery(crate::terminal_host_appearance::Query),
    WindowTitle(Option<orchbee_workbench_protocol::WindowTitle>),
    Notification(crate::notifications::Effect),
    /// Bracketed-paste lease for modal capture.
    BracketedPaste(bool),
    /// One complete outer pointer level, or release of every owned pointer mode.
    PointerCapture(PointerCaptureRequest),
}

/// The interaction facts of the currently attached pane's latest frame —
/// the pane's requested mouse contract plus its screen/cursor modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PointerFrameFacts {
    pub(crate) tracking: MouseTracking,
    pub(crate) format: MouseFormat,
    /// `(cell_pixel_width, cell_pixel_height)`; `(0, 0)` means the host
    /// reported pixels unknown — cell-mode input still works.
    pub(crate) cell_pixel: (u16, u16),
    pub(crate) alternate_screen: bool,
    pub(crate) alternate_scroll: bool,
    pub(crate) application_cursor: bool,
}

/// A terminal cell inside the pane viewport recorded by the pointer press.
/// This stays deliberately independent from the Host wire coordinates: the
/// native session resolves the exact attachment and geometry only after it
/// admits the semantic start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PointerSelectionCell {
    pub(crate) column: u16,
    pub(crate) row: u16,
}

/// One Client-side selection gesture step. The pointer layer owns cells and
/// ordering; the native session remains the sole authority for translating
/// them into a Host selection command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerSelectionIntent {
    Start {
        click_count: u8,
        cell: PointerSelectionCell,
    },
    Update {
        cell: PointerSelectionCell,
    },
    End {
        cell: PointerSelectionCell,
    },
    Cancel,
}

/// Whether the native session admitted a selection start for the exact pane.
/// Ports that do not implement selection reject it, so adding this private
/// seam cannot silently claim ownership in an older backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerSelectionAdmission {
    Admitted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerLinkAdmission {
    Admitted,
    Busy,
    Rejected,
}

/// One tab's stable identity plus its 1-based position — what a recorded
/// tab region resolves against at click time. Pane identity inside the
/// tab never resolves positionally: hit regions carry exact `PaneRef`
/// values, so the probe needs no panel list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PointerTab {
    pub(crate) id: WorkbenchTabId,
    /// 1-based position, matching `ClientAction::SelectTab`.
    pub(crate) position: usize,
}

/// The live facts of one pane leaf the last probe observed, keyed by the
/// record's exact incarnation. Identity is never positional: a pending
/// gesture compares its recorded `PaneRef` against `record` bit for bit —
/// a fresh incarnation of the same `(anchor, panel)` fails the equality
/// and orphans the gesture rather than substituting the new attachment.
#[derive(Clone, Debug)]
pub(crate) struct PointerPane {
    pub(crate) right_click_reporting: bool,
    pub(crate) right_click: orchbee_workbench_protocol::PanelRightClickTarget,
    /// The record's exact incarnation (logical pane + incarnation).
    pub(crate) record: crate::workbench_client::state::PaneRef,
    /// The record's attachment is live for this incarnation.
    pub(crate) attached: bool,
    /// The record's controller mode once known: `Some(true)` admits input
    /// (`Controlling`), `Some(false)` adjudicates negatively (`ReadOnly`),
    /// and `None` means the mode has not arrived yet — a settling focus
    /// must keep waiting rather than cancel as read-only.
    pub(crate) controlling: Option<bool>,
    /// The record's installed projection reached `FrameAvailable` for
    /// this exact incarnation — a usable frame from the same generation,
    /// so an attached local focus settles immediately rather than waiting
    /// on a newer sequence.
    pub(crate) frame_available: bool,
}

/// The session facts a pointer event resolves against, probed fresh per
/// event so hit decisions always see current state — never a stale
/// snapshot from paint time.
#[derive(Clone, Default)]
pub(crate) struct PointerProbe {
    /// A modal draft or an armed confirmation owns input; pointer events
    /// are swallowed without producing gestures.
    pub(crate) input_captured: bool,
    /// The focus pane's exact live incarnation — `None` when the focus
    /// pane departed or has no record. Compared bit for bit with a
    /// gesture's recorded target.
    pub(crate) focus: Option<crate::workbench_client::state::PaneRef>,
    /// The selection's desired pane key — its anchor names the active tab
    /// for stale-map edge resolution.
    pub(crate) desired: Option<crate::workbench_client::state::PaneKey>,
    /// Per-leaf live facts for every pane record, keyed by exact
    /// incarnation — the pending settler looks up its recorded target.
    pub(crate) panes: Vec<PointerPane>,
    pub(crate) unbound: Vec<PointerUnboundPane>,
    /// `forward` would accept input right now: Ready phase under a
    /// Controlling mode with an exact focus incarnation that no close
    /// suppresses. `focus == expected` alone is not this — the phase and
    /// mode must be live reads, not a painted snapshot.
    pub(crate) input_ready: bool,
    /// The focus pane can admit a Host-backed selection right now: Ready
    /// phase with read authority for the exact focus incarnation and no
    /// close or retirement suppression. This is independent from
    /// `input_ready`; a ReadOnly viewer can still select and copy text.
    pub(crate) selection_ready: bool,
    /// Tabs in painted order for stable-id → position resolution.
    pub(crate) tabs: Vec<PointerTab>,
    /// The focus pane's current interaction facts; `None` while no
    /// current frame exists for the focused incarnation.
    pub(crate) frame: Option<PointerFrameFacts>,
    /// The focus pane currently owns a non-Live retained-history core:
    /// wheel gestures over it scroll locally instead of encoding to the
    /// pane. Read fresh per event, like every other probe fact.
    pub(crate) viewport_non_live: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PointerUnboundPane {
    pub(crate) key: crate::workbench_client::state::PaneKey,
    pub(crate) presentation: orchbee_workbench_protocol::TerminalPresentationId,
}

impl PointerProbe {
    /// Resolves a recorded tab identity to its current 0-based index.
    pub(crate) fn tab_index(&self, id: &WorkbenchTabId) -> Option<usize> {
        self.tabs.iter().position(|tab| &tab.id == id)
    }
}

pub(crate) trait TerminalInputPort {
    fn copy_picker_identifier(&mut self, _text: String) -> bool {
        false
    }

    /// Pure exclusive-capture intents from the native surface owners.
    fn native_capture_claims(&self) -> CaptureClaims {
        CaptureClaims::default()
    }

    /// Exact per-item context for the compositor-selected native owner.
    fn native_capture_context(&self, _owner: WorkbenchCaptureOwner) -> Option<CaptureContext> {
        None
    }

    fn click_resize_control(
        &mut self,
        _target: &crate::tui_layout::DrawnResize,
        _direction: crate::client_view::PanelDirection,
        _fine: bool,
    ) {
    }
    /// Validate a completed tab-create click; the ordinary action path owns its effects.
    fn tab_create_action(
        &mut self,
        _target: &crate::action_menu::DrawnTabCreate,
    ) -> Option<ClientAction> {
        None
    }
    fn capture_settings_key_for(
        &mut self,
        _expected: crate::native_settings::SettingsInputContext,
        _event: crate::client_view::KeyEvent,
    ) -> bool {
        false
    }
    fn capture_settings_paste_for(
        &mut self,
        _expected: crate::native_settings::SettingsInputContext,
        _text: Option<&str>,
    ) -> bool {
        false
    }
    fn click_settings_control(
        &mut self,
        _token: crate::native_settings::Token,
        _control: crate::native_settings::Control,
    ) {
    }
    fn scroll_settings(
        &mut self,
        _token: crate::native_settings::Token,
        _up: bool,
        _lines: usize,
    ) -> Option<crate::native_settings::Token> {
        None
    }

    fn click_settings(
        &mut self,
        _token: crate::native_settings::Token,
        _row: crate::native_settings::Row,
    ) {
    }
    fn open_action_menu(
        &mut self,
        _target: crate::action_menu::MenuTarget,
        _anchor: Option<(u16, u16)>,
        _pointer_released: bool,
    ) -> bool {
        false
    }
    fn capture_action_menu_key_for(
        &mut self,
        _expected: crate::action_menu::MenuInputContext,
        _event: crate::client_view::KeyEvent,
    ) -> bool {
        false
    }
    fn hover_action_menu_row(
        &mut self,
        _source: &crate::action_menu::MenuTarget,
        _token: crate::action_menu::MenuToken,
        _row: usize,
    ) -> bool {
        false
    }
    fn activate_action_menu_row(
        &mut self,
        _source: &crate::action_menu::MenuTarget,
        _token: crate::action_menu::MenuToken,
        _row: usize,
    ) {
    }
    fn activate_action_menu_control(
        &mut self,
        _source: &crate::action_menu::MenuTarget,
        _token: crate::action_menu::MenuToken,
        _control: crate::action_menu::MenuControl,
    ) {
    }
    fn scroll_action_menu(
        &mut self,
        _token: crate::action_menu::MenuToken,
        _up: bool,
        _lines: usize,
    ) -> Option<crate::action_menu::MenuToken> {
        None
    }
    fn cancel_action_menu(&mut self) {}
    fn take_action_menu_action(&mut self, _pointer_released: bool) -> Option<ClientAction> {
        None
    }
    fn observe_host_graphics(&mut self, _facts: crate::terminal_graphics::Facts) {}
    fn popup_input(
        &mut self,
        _id: &orchbee_workbench_protocol::WorkbenchRequestId,
        _item: &DecodedItem,
    ) {
    }
    fn popup_mouse(
        &mut self,
        _id: &orchbee_workbench_protocol::WorkbenchRequestId,
        _content: ratatui::layout::Rect,
        _event: MouseEvent,
    ) {
    }
    fn activate_drawn_popup_close(
        &mut self,
        _target: &crate::workbench_client::DrawnPopupClose,
    ) -> bool {
        false
    }

    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable>;

    /// Called only after terminal-bound editing input was admitted by `forward`.
    fn terminal_edited(&mut self) {}

    fn navigator_catalog(&self) -> Option<crate::client_view::navigator::Catalog> {
        None
    }

    fn activate_navigator(
        &mut self,
        _row: &crate::client_view::navigator::Row,
    ) -> crate::client_view::navigator::Admission {
        crate::client_view::navigator::Admission::Refused
    }

    fn activate_drawn_presentation(
        &mut self,
        _target: &crate::workbench_client::DrawnPresentationTarget,
    ) -> ClientActionDisposition {
        ClientActionDisposition::KeepNavigator
    }

    /// Decoded human intent, before routing; host metadata and captured tails are excluded.
    fn observe_input_intent(&mut self) {}

    /// The current pointer-resolution facts. Ports without a session
    /// report the default: nothing captured, nothing attached, no regions
    /// resolvable — every gesture adjudicates as dead space.
    fn pointer_probe(&mut self) -> PointerProbe {
        PointerProbe::default()
    }

    fn sidebar_reorder_source(&mut self) -> Option<crate::tui_pointer::SidebarReorderSource> {
        None
    }
    fn commit_sidebar_reorder(
        &mut self,
        _source: &crate::tui_pointer::SidebarReorderSource,
        _operation: orchbee_workbench_protocol::NavigationConfiguration,
    ) {
    }

    fn tab_drag_source(&mut self) -> Option<crate::tui_pointer::TabDragSource> {
        None
    }

    fn request_drawn_tab_rename(
        &mut self,
        _source: &crate::action_menu::DrawnTabCreate,
        _tab: &orchbee_workbench_protocol::WorkbenchTabId,
    ) -> Option<ClientActionDisposition> {
        None
    }

    fn request_drawn_tab_close(
        &mut self,
        _source: &crate::action_menu::DrawnTabCreate,
        _tab: &orchbee_workbench_protocol::WorkbenchTabId,
    ) {
    }

    fn commit_tab_drag(
        &mut self,
        _source: &crate::action_menu::DrawnTabCreate,
        _operation: orchbee_workbench_protocol::TabConfiguration,
    ) {
    }

    /// Revalidates the full painted source without taking input control or submitting work.
    fn pointer_divider_ready(
        &mut self,
        _target: &crate::tui_layout::DrawnDivider,
    ) -> Result<(), crate::client_view::Notice> {
        Err(crate::client_view::Notice::ResizeFailed)
    }

    /// A read-only presentation check neither admits a mutation nor publishes busy feedback.
    fn divider_projection_current(
        &self,
        _target: &crate::tui_layout::DrawnDivider,
        _viewport: (ratatui::layout::Rect, crate::client_view::SidebarSurface),
    ) -> bool {
        false
    }

    /// Hands off one changed release; the asynchronous local owner reports its eventual result.
    fn pointer_divider_resize(
        &mut self,
        _target: &crate::tui_layout::DrawnDivider,
        _ratio: orchbee_workbench_protocol::ClientSplitRatio,
    ) {
    }

    /// The local acknowledgement owner may retain released geometry until canonical publication.
    /// A withdrawn presentation lease is retired independently of the submitted mutation.
    fn pending_resize_projection(
        &mut self,
        _viewport: ratatui::layout::Rect,
        _sidebar: crate::client_view::SidebarSurface,
        _presentation_allowed: bool,
    ) -> crate::tui_layout::ResizeProjection {
        crate::tui_layout::ResizeProjection::Canonical
    }

    /// Sends one cell-semantic selection step for the exact painted pane.
    /// Only `Start` has an admission decision; callers ignore the return for
    /// later steps after a start was admitted. The default is closed.
    fn pointer_selection(
        &mut self,
        _pane: &crate::workbench_client::state::PaneRef,
        _intent: PointerSelectionIntent,
    ) -> PointerSelectionAdmission {
        PointerSelectionAdmission::Rejected
    }

    /// Admit one Ctrl+click from an exact successfully painted pane source.
    /// This read-only path never focuses the pane or acquires terminal control.
    fn pointer_link(
        &mut self,
        _source: crate::terminal_link_activation::PaintedLinkSource,
        _cell: PointerSelectionCell,
    ) -> PointerLinkAdmission {
        PointerLinkAdmission::Rejected
    }

    fn apply_client_action(&mut self, _action: ClientAction) -> Option<ClientActionDisposition> {
        None
    }

    /// Whether the focus pane's non-Live history core consumes this key
    /// locally. Ports without history always report `false`: the event
    /// then follows the ordinary byte path untouched.
    fn history_key(&mut self, _event: &crate::client_view::KeyEvent) -> bool {
        false
    }

    fn copy_mode_active(&self) -> bool {
        false
    }

    fn click_copy_control(
        &mut self,
        _token: crate::terminal_copy_mode::CopyModeToken,
        _control: crate::terminal_copy_mode::CopyControl,
    ) {
    }
    fn copy_pointer_cell(
        &mut self,
        _token: crate::terminal_copy_mode::CopySurfaceToken,
        _cell: crate::terminal_copy_mode::CopyModeCursor,
        _start: bool,
    ) -> bool {
        false
    }
    fn scroll_copy_mode(
        &mut self,
        _token: crate::terminal_copy_mode::CopyModeToken,
        _up: bool,
        _lines: usize,
    ) -> Option<crate::terminal_copy_mode::CopyModeToken> {
        None
    }

    fn capture_copy_mode_key(&mut self, _event: &crate::client_view::KeyEvent) -> bool {
        false
    }

    fn capture_copy_mode_paste(&mut self, _text: Option<&str>) -> bool {
        false
    }

    fn cancel_copy_mode(&mut self) -> bool {
        false
    }

    /// Whether a wheel notch over `key` is consumed by that pane's
    /// non-Live history core (three rows per notch). The pointer layer
    /// only calls this for the exact focus incarnation it already probed.
    fn history_wheel(&mut self, _key: &crate::workbench_client::state::PaneRef, _up: bool) -> bool {
        false
    }

    /// Revalidates the exact painted source before admitting a bounded tail jump.
    fn scrollbar_scroll(
        &mut self,
        _key: &crate::workbench_client::state::PaneRef,
        _source: crate::terminal_scrollbar::ScrollbarSnapshot,
        _offset: u32,
    ) -> bool {
        false
    }

    fn observe_geometry(&mut self, _geometry: Geometry) {}

    /// Host focus is metadata; ports without a native Client consume it.
    fn observe_focus(&mut self, _focused: bool) {}

    fn capture_tab_confirmation_key_for(
        &mut self,
        _token: crate::workbench_client::CloseConfirmationToken,
        _key: ClientKey,
    ) -> bool {
        false
    }

    fn capture_panel_confirmation_key_for(
        &mut self,
        _token: crate::workbench_client::PanelCloseConfirmationToken,
        _key: ClientKey,
    ) -> bool {
        false
    }

    fn capture_workspace_confirmation_key_for(
        &mut self,
        _token: crate::workbench_client::WorkspaceCloseToken,
        _key: ClientKey,
    ) -> bool {
        false
    }

    /// A completed click on an exact, fully painted Workspace/Project close decision.
    fn click_tab_close(
        &mut self,
        _token: crate::workbench_client::CloseConfirmationToken,
        _confirm: bool,
    ) {
    }

    fn click_panel_close(
        &mut self,
        _token: crate::workbench_client::PanelCloseConfirmationToken,
        _confirm: bool,
    ) {
    }

    fn click_workspace_close(
        &mut self,
        _token: crate::workbench_client::WorkspaceCloseToken,
        _confirm: bool,
    ) {
    }

    fn capture_agent_board_key_for(
        &mut self,
        _episode: crate::workbench_client::AgentBoardEpisodeToken,
        _key: crate::client_view::KeyEvent,
    ) -> bool {
        false
    }

    /// A press on a painted board row — resolves by stable session
    /// identity, never by position.
    fn agent_board_select(
        &mut self,
        _target: &crate::workbench_client::AgentBoardRowTarget,
    ) -> bool {
        false
    }

    fn scroll_agent_board(
        &mut self,
        _token: &crate::workbench_client::AgentBoardToken,
        _up: bool,
        _lines: usize,
    ) -> Option<crate::workbench_client::AgentBoardToken> {
        None
    }
    fn click_agent_board_control(
        &mut self,
        _target: &crate::workbench_client::AgentBoardControlTarget,
    ) -> bool {
        false
    }

    /// An outside press dismissed the board's capturing overlay.
    fn agent_board_dismiss(&mut self) {}

    fn invalidate_close_display(&mut self) {}

    fn edit_rename_draft(&mut self, _token: RenameDraftToken, _edit: RenameEdit<'_>) -> bool {
        false
    }

    fn cancel_rename_draft(&mut self, _token: RenameDraftToken) {}

    fn submit_rename_draft(&mut self, _token: RenameDraftToken) -> Option<ClientActionDisposition> {
        None
    }

    fn invalidate_rename_display(&mut self) {}

    fn edit_worktree(&mut self, _token: WorktreeToken, _edit: WorktreeEdit<'_>) -> bool {
        false
    }
    fn dismiss_worktree(&mut self, _token: WorktreeToken) {}
    fn submit_worktree(&mut self, _token: WorktreeToken) {}
    /// Submit an exact checkout from the current successfully painted picker.
    fn choose_worktree(&mut self, _token: WorktreeToken, _index: usize) {}
    /// Move the picker only while its painted token still matches.
    fn scroll_worktree(
        &mut self,
        _token: WorktreeToken,
        _up: bool,
        _lines: usize,
    ) -> Option<WorktreeToken> {
        None
    }
    fn invalidate_worktree_display(&mut self) {}

    fn edit_setup_draft(&mut self, _token: SetupToken, _edit: SetupEdit<'_>) -> bool {
        false
    }

    fn cancel_setup_draft(&mut self, _token: SetupToken) {}

    fn submit_setup_draft(&mut self, _token: SetupToken) -> Option<ClientActionDisposition> {
        None
    }

    /// Select and submit one exact Project row authorized by the last draw.
    fn choose_setup_project(&mut self, _token: SetupToken, _index: usize) {}

    /// Move the Project picker only while its painted token still matches.
    fn scroll_setup_projects(
        &mut self,
        _token: SetupToken,
        _up: bool,
        _lines: usize,
    ) -> Option<SetupToken> {
        None
    }

    fn invalidate_setup_display(&mut self) {}
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct UnavailableTerminalInputPort;

impl TerminalInputPort for UnavailableTerminalInputPort {
    fn forward(&mut self, _input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        Err(TerminalInputUnavailable)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DispatchOutcome {
    Continue,
    Exit,
}

/// Test helper: drives the real `InputDispatcher` path so tests exercise the
/// same decode → ownership → dispatch rules as production, instead of a
/// parallel byte-level loop.
#[cfg(test)]
pub(crate) fn dispatch_input<Port: TerminalInputPort + ?Sized, LayoutError>(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    input: &[u8],
    terminal_input: &mut Port,
    now: std::time::Instant,
    mut after_layout_change: impl FnMut(&mut ClientView, &mut Port) -> Result<(), LayoutError>,
) -> Result<DispatchOutcome, LayoutError> {
    dispatcher.dispatch(
        view,
        input,
        terminal_input,
        now,
        |view, port| after_layout_change(view, port).map(|()| false),
        &mut |_| Ok(()),
    )
}

pub(crate) fn forward_or_report(
    view: &mut ClientView,
    input: &[u8],
    terminal_input: &mut (impl TerminalInputPort + ?Sized),
) {
    if terminal_input.forward(input).is_err() {
        view.report_terminal_input_unavailable();
    }
}

fn forward_edit_or_report(
    view: &mut ClientView,
    input: &[u8],
    editing: bool,
    terminal_input: &mut (impl TerminalInputPort + ?Sized),
) {
    match terminal_input.forward(input) {
        Ok(()) if editing => terminal_input.terminal_edited(),
        Ok(()) => {}
        Err(_) => view.report_terminal_input_unavailable(),
    }
}

#[cfg(test)]
mod tests;
