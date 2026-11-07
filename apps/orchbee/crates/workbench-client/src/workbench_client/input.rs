//! Workbench input capabilities and current pointer/capture facts. The input dispatcher consumes
//! this contract; the port owns no byte decoder, terminal lifecycle writer or dispatch loop.

mod action;
mod capture;
mod drag_sources;
mod surface;
pub(crate) use action::CapturedClientAction;
pub(in crate::workbench_client) use action::CapturedRequest;
pub use action::InputAction;
pub use capture::{CaptureContext, RenameCaptureContext, RenameDraftToken, RenameEdit};
pub(crate) use capture::{
    CaptureReadiness, ConfirmationCaptureContext, RenameInputContext, SetupCaptureContext,
    WorktreeCaptureContext,
};
pub(crate) use drag_sources::TabDragTab;
pub use drag_sources::{SidebarReorderSource, TabDragSource};
pub use surface::{
    BoardAction, CloseDecision, CopyAction, LayoutAction, MenuAction, RenameAction, SettingsAction,
    SetupAction, SurfaceAction, SurfaceInput, TabAction, TextEntry, WorktreeAction,
};

use crate::client_view::{ClientAction, ClientActionDisposition};
use crate::tui_composition::{CaptureClaims, WorkbenchCaptureOwner};
use orchbee_client_server_protocol::Geometry;
use orchbee_terminal_protocol::{MouseFormat, MouseTracking};
use orchbee_workbench_protocol::WorkbenchTabId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalInputUnavailable;

/// The complete outer-terminal pointer reporting state requested by input.
/// The shell publishes the matching dispatcher state only after the lifecycle
/// write succeeds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PointerCaptureRequest {
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
pub enum ModeRequest {
    /// One pane bell, emitted after the completed frame has flushed.
    Bell,
    /// Match admitted outer-terminal padding to the current canvas; None restores it.
    HostBackground(Option<u32>),
    /// Bounded metadata queries on the already-admitted host terminal.
    GraphicsQuery(crate::terminal_graphics::Query),
    /// The endpoint's sole any-motion capability query, after ButtonOnly is active.
    PointerAdmissionQuery(crate::terminal_pointer_admission::Query),
    /// One bounded OSC 10/11 host-default color query at terminal admission.
    AppearanceQuery(crate::terminal_host_appearance::Query),
    WindowTitle(Option<orchbee_workbench_protocol::WindowTitle>),
    Notification(crate::client_view::notifications::Effect),
    /// Bracketed-paste lease for modal capture.
    BracketedPaste(bool),
    /// One complete outer pointer level, or release of every owned pointer mode.
    PointerCapture(PointerCaptureRequest),
}

/// Host and input-stream facts the port observes. None of them is a gesture or a command, and a
/// port without a native Client consumes every one of them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalObservation {
    /// Host focus is metadata; it never selects an input owner.
    Focus(bool),
    /// The host window geometry the shell measured for the next frame.
    Geometry(Geometry),
    /// Settled host graphics facts.
    HostGraphics(crate::terminal_graphics::Facts),
    /// Decoded human intent, before routing; host metadata and captured tails are excluded.
    InputIntent,
    /// Input routed to the terminal proves keyboard delivery even if FocusIn arrives later.
    KeyboardIntent,
    /// The host window changed size: painted close, rename, setup and worktree decisions stay
    /// stale until their next draw.
    DisplayInvalidated,
}

/// The interaction facts of the currently attached pane's latest frame —
/// the pane's requested mouse contract plus its screen/cursor modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerFrameFacts {
    pub(crate) tracking: MouseTracking,
    pub(crate) format: MouseFormat,
    /// `(cell_pixel_width, cell_pixel_height)`; `(0, 0)` means the host
    /// reported pixels unknown — cell-mode input still works.
    pub(crate) cell_pixel: (u16, u16),
    pub(crate) alternate_screen: bool,
    pub(crate) alternate_scroll: bool,
    pub(crate) application_cursor: bool,
}

pub use super::terminal_selection::{PointerSelectionCell, PointerSelectionIntent};
pub use crate::tui_hit_map::{DrawnPresentationTarget, PointerUnboundPane};

/// Input that the focused pane's own viewport owns: provisional selection, link activation and
/// non-Live history movement. Each step names the exact painted incarnation it resolves against.
pub enum PaneInput<'a> {
    /// One cell-semantic selection step for the exact painted pane. Only `Start` has an admission
    /// decision; callers ignore the result of later steps after an admitted start.
    Selection {
        pane: &'a crate::workbench_state::PaneRef,
        intent: PointerSelectionIntent,
    },
    /// One Ctrl+click from an exact successfully painted pane source. This read-only path never
    /// focuses the pane or acquires terminal control.
    Link {
        source: Box<crate::terminal_link_activation::PaintedLinkSource>,
        cell: PointerSelectionCell,
    },
    /// A key that the focus pane's non-Live history core may consume locally. A rejected key
    /// follows the ordinary byte path untouched.
    HistoryKey(&'a crate::client_view::KeyEvent),
    /// A wheel notch (three rows) over the non-Live history core of the exact focus incarnation
    /// the pointer already probed.
    HistoryWheel {
        pane: &'a crate::workbench_state::PaneRef,
        up: bool,
    },
    /// A bounded tail jump; the pane revalidates the exact painted scrollbar source first.
    Scrollbar {
        pane: &'a crate::workbench_state::PaneRef,
        source: crate::terminal_scrollbar::ScrollbarSnapshot,
        offset: u32,
    },
}

/// Whether the pane admitted one [`PaneInput`]. Ports without pane input reject every step, so a
/// backend that never implemented one cannot silently claim its ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneAdmission {
    Admitted,
    /// An earlier link activation still owns the effect; only `PaneInput::Link` reports it.
    Busy,
    Rejected,
}

/// One tab's stable identity plus its 1-based position — what a recorded
/// tab region resolves against at click time. Pane identity inside the
/// tab never resolves positionally: hit regions carry exact `PaneRef`
/// values, so the probe needs no panel list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerTab {
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
pub struct PointerPane {
    pub(crate) right_click_reporting: bool,
    pub(crate) right_click: orchbee_workbench_protocol::PanelRightClickTarget,
    /// The record's exact incarnation (logical pane + incarnation).
    pub(crate) record: crate::workbench_state::PaneRef,
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
pub struct PointerProbe {
    /// A modal draft or an armed confirmation owns input; pointer events
    /// are swallowed without producing gestures.
    pub(crate) input_captured: bool,
    /// The focus pane's exact live incarnation — `None` when the focus
    /// pane departed or has no record. Compared bit for bit with a
    /// gesture's recorded target.
    pub(crate) focus: Option<crate::workbench_state::PaneRef>,
    /// The selection's desired pane key — its anchor names the active tab
    /// for stale-map edge resolution.
    pub(crate) desired: Option<crate::workbench_state::PaneKey>,
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

impl PointerProbe {
    /// Resolves a recorded tab identity to its current 0-based index.
    pub(crate) fn tab_index(&self, id: &WorkbenchTabId) -> Option<usize> {
        self.tabs.iter().position(|tab| &tab.id == id)
    }
}

/// The only way `input` modules reach composition. Entries come in four groups, in this order:
/// facts that dispatch and gesture resolution read; terminal forwarding and host observations;
/// local input that stays with the owner of its target; and committed results, either a
/// feature-defined [`SurfaceAction`] or a Client action resolved from bindings, menus, the
/// navigator, or a picker. A value aimed at a painted or captured incarnation carries its witness,
/// and the owner refuses a stale one. The crate's `docs/module-layers.md` lists the invariants
/// every implementation keeps.
pub trait TerminalInputPort {
    /// Pure exclusive-capture intents from the native surface owners.
    fn native_capture_claims(&self) -> CaptureClaims {
        CaptureClaims::default()
    }

    /// Exact per-item context for the compositor-selected native owner.
    fn native_capture_context(&self, _owner: WorkbenchCaptureOwner) -> Option<CaptureContext> {
        None
    }

    fn copy_mode_active(&self) -> bool {
        false
    }

    /// The current pointer-resolution facts. Ports without a session
    /// report the default: nothing captured, nothing attached, no regions
    /// resolvable — every gesture adjudicates as dead space.
    fn pointer_probe(&mut self) -> PointerProbe {
        PointerProbe::default()
    }

    fn sidebar_reorder_source(&mut self) -> Option<SidebarReorderSource> {
        None
    }

    fn tab_drag_source(&mut self) -> Option<TabDragSource> {
        None
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

    fn navigator_catalog(&self) -> Option<crate::client_view::navigator::Catalog> {
        None
    }

    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable>;

    /// Whether a requested navigation that changes the focused terminal has not landed yet.
    /// Focus still names the pane being left, so passthrough input belongs to no terminal.
    fn navigation_pending(&self) -> bool {
        false
    }

    /// A pointer press reached no target because no presented frame could resolve it. Returns
    /// whether the user's own switch is landing: the press was then aimed at the frame that switch
    /// keeps on screen, and the switch's destination no longer names the user's latest intent.
    fn unresolved_press(&mut self) -> bool {
        false
    }

    /// One host or input-stream fact; see [`TerminalObservation`].
    fn observe_terminal(&mut self, _observation: TerminalObservation) {}

    /// One pane-local input; see [`PaneInput`]. The default rejects it.
    fn pane_input(&mut self, _input: PaneInput<'_>) -> PaneAdmission {
        PaneAdmission::Rejected
    }

    /// One local input for the surface that holds input; see [`SurfaceInput`]. Returns whether
    /// the owner consumed it or changed. The default consumes nothing.
    fn surface_input(&mut self, _input: SurfaceInput<'_>) -> bool {
        false
    }

    /// Moves one painted list while its painted token still matches and returns the list's new
    /// painted target. View-owned lists (help, navigator, machine picker) never reach the port,
    /// and the default moves nothing.
    fn scroll_list(
        &mut self,
        _target: &crate::tui_hit_map::ListScrollTarget,
        _up: bool,
        _lines: usize,
    ) -> Option<crate::tui_hit_map::ListScrollTarget> {
        None
    }

    /// One committed gesture result; see [`SurfaceAction`]. Returns the layout disposition the
    /// owner reports, or `None` when it reports none or refused a stale witness. The default
    /// commits nothing.
    fn surface_action(&mut self, _action: SurfaceAction<'_>) -> Option<ClientActionDisposition> {
        None
    }

    fn apply_client_action(&mut self, _action: InputAction) -> Option<ClientActionDisposition> {
        None
    }

    /// Validate a completed tab-create click; the ordinary action path owns its effects.
    fn tab_create_action(
        &mut self,
        _target: &crate::action_menu::DrawnTabCreate,
    ) -> Option<ClientAction> {
        None
    }

    /// Where the sidebar row of the menu about to open stands, while it shows Git facts for it.
    fn observe_sidebar_git(
        &mut self,
        _directory: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
    ) {
    }

    fn open_action_menu(
        &mut self,
        _target: crate::action_menu::MenuTarget,
        _anchor: Option<crate::action_menu::MenuAnchor>,
        _pointer_released: bool,
    ) -> bool {
        false
    }

    fn take_action_menu_action(&mut self, _pointer_released: bool) -> Option<InputAction> {
        None
    }

    fn activate_navigator(
        &mut self,
        _row: &crate::client_view::navigator::Row,
    ) -> crate::client_view::navigator::Admission {
        crate::client_view::navigator::Admission::Refused
    }

    fn copy_picker_identifier(&mut self, _text: String) -> bool {
        false
    }

    /// Opens the Agent target a visible notification card announced, revalidated against the
    /// current Workbench. The default opens nothing.
    fn open_notification_target(
        &mut self,
        _target: &crate::machine_agents::Target,
    ) -> crate::client_view::notifications::TargetOpening {
        crate::client_view::notifications::TargetOpening::Unavailable
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableTerminalInputPort;

impl TerminalInputPort for UnavailableTerminalInputPort {
    fn forward(&mut self, _input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        Err(TerminalInputUnavailable)
    }
}

/// Crate-internal `InputAction` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait InputActionTestAccess {
    fn intent(&self) -> ClientAction;
}

#[cfg(any(test, feature = "test-support"))]
impl InputActionTestAccess for InputAction {
    fn intent(&self) -> ClientAction {
        InputAction::intent(self)
    }
}

/// Crate-internal `PointerProbe` state the application's tests read and adjust.
#[cfg(any(test, feature = "test-support"))]
pub trait PointerProbeTestAccess {
    fn panes(&self) -> &[PointerPane];
    fn panes_mut(&mut self) -> &mut Vec<PointerPane>;
    fn focus_mut(&mut self) -> &mut Option<crate::workbench_state::PaneRef>;
    fn desired_mut(&mut self) -> &mut Option<crate::workbench_state::PaneKey>;
    fn frame_mut(&mut self) -> &mut Option<PointerFrameFacts>;
}

#[cfg(any(test, feature = "test-support"))]
impl PointerProbeTestAccess for PointerProbe {
    fn panes(&self) -> &[PointerPane] {
        &self.panes
    }
    fn panes_mut(&mut self) -> &mut Vec<PointerPane> {
        &mut self.panes
    }
    fn focus_mut(&mut self) -> &mut Option<crate::workbench_state::PaneRef> {
        &mut self.focus
    }
    fn desired_mut(&mut self) -> &mut Option<crate::workbench_state::PaneKey> {
        &mut self.desired
    }
    fn frame_mut(&mut self) -> &mut Option<PointerFrameFacts> {
        &mut self.frame
    }
}

/// Crate-internal `PointerPane` state the application's tests read and adjust.
#[cfg(any(test, feature = "test-support"))]
pub trait PointerPaneTestAccess {
    fn record(&self) -> &crate::workbench_state::PaneRef;
    fn attached(&self) -> bool;
    fn frame_available_mut(&mut self) -> &mut bool;
}

#[cfg(any(test, feature = "test-support"))]
impl PointerPaneTestAccess for PointerPane {
    fn record(&self) -> &crate::workbench_state::PaneRef {
        &self.record
    }
    fn attached(&self) -> bool {
        self.attached
    }
    fn frame_available_mut(&mut self) -> &mut bool {
        &mut self.frame_available
    }
}

/// Field-for-field `PointerFrameFacts` fixture the application's tests build values from.
#[cfg(any(test, feature = "test-support"))]
pub struct PointerFrameFactsFixture {
    pub tracking: MouseTracking,
    pub format: MouseFormat,
    pub cell_pixel: (u16, u16),
    pub alternate_screen: bool,
    pub alternate_scroll: bool,
    pub application_cursor: bool,
}

#[cfg(any(test, feature = "test-support"))]
impl From<PointerFrameFactsFixture> for PointerFrameFacts {
    fn from(fixture: PointerFrameFactsFixture) -> Self {
        let PointerFrameFactsFixture {
            tracking,
            format,
            cell_pixel,
            alternate_screen,
            alternate_scroll,
            application_cursor,
        } = fixture;
        Self {
            tracking,
            format,
            cell_pixel,
            alternate_screen,
            alternate_scroll,
            application_cursor,
        }
    }
}

/// Field-for-field `PointerTab` fixture the application's tests build values from.
#[cfg(any(test, feature = "test-support"))]
pub struct PointerTabFixture {
    pub id: WorkbenchTabId,
    pub position: usize,
}

#[cfg(any(test, feature = "test-support"))]
impl From<PointerTabFixture> for PointerTab {
    fn from(fixture: PointerTabFixture) -> Self {
        let PointerTabFixture { id, position } = fixture;
        Self { id, position }
    }
}

/// Field-for-field `PointerPane` fixture the application's tests build values from.
#[cfg(any(test, feature = "test-support"))]
pub struct PointerPaneFixture {
    pub right_click_reporting: bool,
    pub right_click: orchbee_workbench_protocol::PanelRightClickTarget,
    pub record: crate::workbench_state::PaneRef,
    pub attached: bool,
    pub controlling: Option<bool>,
    pub frame_available: bool,
}

#[cfg(any(test, feature = "test-support"))]
impl From<PointerPaneFixture> for PointerPane {
    fn from(fixture: PointerPaneFixture) -> Self {
        let PointerPaneFixture {
            right_click_reporting,
            right_click,
            record,
            attached,
            controlling,
            frame_available,
        } = fixture;
        Self {
            right_click_reporting,
            right_click,
            record,
            attached,
            controlling,
            frame_available,
        }
    }
}

/// Field-for-field `PointerProbe` fixture the application's tests build values from.
#[cfg(any(test, feature = "test-support"))]
#[derive(Default)]
pub struct PointerProbeFixture {
    pub input_captured: bool,
    pub focus: Option<crate::workbench_state::PaneRef>,
    pub desired: Option<crate::workbench_state::PaneKey>,
    pub panes: Vec<PointerPane>,
    pub unbound: Vec<PointerUnboundPane>,
    pub input_ready: bool,
    pub selection_ready: bool,
    pub tabs: Vec<PointerTab>,
    pub frame: Option<PointerFrameFacts>,
    pub viewport_non_live: bool,
}

#[cfg(any(test, feature = "test-support"))]
impl From<PointerProbeFixture> for PointerProbe {
    fn from(fixture: PointerProbeFixture) -> Self {
        let PointerProbeFixture {
            input_captured,
            focus,
            desired,
            panes,
            unbound,
            input_ready,
            selection_ready,
            tabs,
            frame,
            viewport_non_live,
        } = fixture;
        Self {
            input_captured,
            focus,
            desired,
            panes,
            unbound,
            input_ready,
            selection_ready,
            tabs,
            frame,
            viewport_non_live,
        }
    }
}
