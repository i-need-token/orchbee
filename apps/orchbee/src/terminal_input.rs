use crate::client_view::{ClientAction, ClientKey, ClientView, SessionActionDisposition};
use crate::terminal_session::{
    NativeContextLocation, NativeRenameDraftToken, NativeRenameEdit, NativeRenameInputContext,
    NativeSetupEdit, NativeSetupInputContext, NativeSetupStage, NativeSetupToken,
};
use orchbee_client_server_protocol::Geometry;
use orchbee_terminal_protocol::{MouseFormat, MouseTracking};
#[cfg(test)]
use orchbee_workbench_protocol::WorkbenchPanelId;
use orchbee_workbench_protocol::WorkbenchTabId;

mod decoder;
mod dispatcher;
mod modal;
mod mouse;
pub(crate) use dispatcher::InputDispatcher;
pub(crate) use mouse::{
    MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent, PointerOwner,
    encode_cursor_arrows, encode_pane_report, pane_owns_pointer,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalInputUnavailable;

/// One terminal-mode write the dispatcher asks the shell to perform. The
/// dispatcher decides *when*; the lifecycle layer owns the actual bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModeRequest {
    /// Bracketed-paste lease for modal capture.
    BracketedPaste(bool),
    /// Pointer reporting (`?1002` + `?1006`) armed or released.
    PointerCapture(bool),
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
    pub(crate) application_cursor: bool,
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
    pub(crate) record: crate::terminal_session::state::PaneRef,
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
    pub(crate) focus: Option<crate::terminal_session::state::PaneRef>,
    /// The selection's desired pane key — its anchor names the active tab
    /// for stale-map edge resolution.
    pub(crate) desired: Option<crate::terminal_session::state::PaneKey>,
    /// Per-leaf live facts for every pane record, keyed by exact
    /// incarnation — the pending settler looks up its recorded target.
    pub(crate) panes: Vec<PointerPane>,
    /// `forward` would accept input right now: Ready phase under a
    /// Controlling mode with an exact focus incarnation that no close
    /// suppresses. `focus == expected` alone is not this — the phase and
    /// mode must be live reads, not a painted snapshot.
    pub(crate) input_ready: bool,
    /// Tabs in painted order for stable-id → position resolution.
    pub(crate) tabs: Vec<PointerTab>,
    /// Navigator rows in painted order for stable-id → index resolution.
    pub(crate) contexts: Vec<NativeContextLocation>,
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

    /// Resolves a recorded context location to its current 0-based
    /// navigation index.
    pub(crate) fn context_index(&self, location: &NativeContextLocation) -> Option<usize> {
        self.contexts.iter().position(|entry| entry == location)
    }
}

/// Test-only pane identities for the `Legacy` session backend: it owns one
/// attached terminal with no workbench membership, so its hit region and
/// probe agree on a fixed sentinel pair instead of real tab/panel ids.
#[cfg(test)]
pub(crate) fn legacy_pane_tab() -> WorkbenchTabId {
    WorkbenchTabId::parse("0000000000000000000000000000000a").expect("sentinel identity")
}

#[cfg(test)]
pub(crate) fn legacy_pane_panel() -> WorkbenchPanelId {
    WorkbenchPanelId::parse("0000000000000000000000000000000b").expect("sentinel identity")
}

/// The sentinel pane's exact incarnation — fixed `(generation 0,
/// incarnation 0)` so the legacy backend's probe and the renderer's hit
/// region agree bit for bit.
#[cfg(test)]
pub(crate) fn legacy_pane_ref() -> crate::terminal_session::state::PaneRef {
    use crate::terminal_session::state::{AttachmentKey, PaneKey, PaneRef};
    PaneRef::new(
        PaneKey::new(
            AttachmentKey::new(0, legacy_pane_tab()),
            legacy_pane_panel(),
        ),
        0,
    )
}

pub(crate) trait TerminalInputPort {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable>;

    /// The current pointer-resolution facts. Ports without a session
    /// report the default: nothing captured, nothing attached, no regions
    /// resolvable — every gesture adjudicates as dead space.
    fn pointer_probe(&mut self) -> PointerProbe {
        PointerProbe::default()
    }

    fn apply_client_action(&mut self, _action: ClientAction) -> Option<SessionActionDisposition> {
        None
    }

    /// Whether the focus pane's non-Live history core consumes this key
    /// locally. Ports without history always report `false`: the event
    /// then follows the ordinary byte path untouched.
    fn history_key(&mut self, _event: &crate::client_view::KeyEvent) -> bool {
        false
    }

    /// Whether a wheel notch over `key` is consumed by that pane's
    /// non-Live history core (three rows per notch). The pointer layer
    /// only calls this for the exact focus incarnation it already probed.
    fn history_wheel(&mut self, _key: &crate::terminal_session::state::PaneRef, _up: bool) -> bool {
        false
    }

    fn observe_geometry(&mut self, _geometry: Geometry) {}

    fn capture_confirmation_key(&mut self, _key: ClientKey) -> bool {
        false
    }

    /// Whether the session's Agent board currently owns input — the
    /// painted-verdict flag the dispatcher consults between the armed
    /// prefix and the pane-forward layer.
    fn agent_board_input(&mut self) -> bool {
        false
    }

    /// One decoded key while the board owns input. Consumed regardless of
    /// outcome — capture guarantees nothing typed reaches the pane.
    fn capture_agent_board_key(&mut self, _key: crate::client_view::KeyEvent) -> bool {
        false
    }

    /// A press on a painted board row — resolves by stable session
    /// identity, never by position.
    fn agent_board_select(
        &mut self,
        _session: &orchbee_agent_observation_protocol::AgentSessionId,
    ) -> bool {
        false
    }

    /// A wheel notch over the board surface — scrolls its row window.
    fn agent_board_scroll(&mut self, _up: bool) -> bool {
        false
    }

    /// An outside press dismissed the board's capturing overlay.
    fn agent_board_dismiss(&mut self) {}

    fn invalidate_close_display(&mut self) {}

    fn rename_input_context(&mut self) -> Option<NativeRenameInputContext> {
        None
    }

    fn edit_rename_draft(
        &mut self,
        _token: NativeRenameDraftToken,
        _edit: NativeRenameEdit<'_>,
    ) -> bool {
        false
    }

    fn cancel_rename_draft(&mut self, _token: NativeRenameDraftToken) {}

    fn submit_rename_draft(
        &mut self,
        _token: NativeRenameDraftToken,
    ) -> Option<SessionActionDisposition> {
        None
    }

    fn invalidate_rename_display(&mut self) {}

    fn setup_input_context(&mut self) -> Option<NativeSetupInputContext> {
        None
    }

    fn edit_setup_draft(&mut self, _token: NativeSetupToken, _edit: NativeSetupEdit<'_>) -> bool {
        false
    }

    fn cancel_setup_draft(&mut self, _token: NativeSetupToken) {}

    fn submit_setup_draft(&mut self, _token: NativeSetupToken) -> Option<SessionActionDisposition> {
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
    after_layout_change: impl FnMut(&mut ClientView, &mut Port) -> Result<(), LayoutError>,
) -> Result<DispatchOutcome, LayoutError> {
    dispatcher.dispatch(
        view,
        input,
        terminal_input,
        now,
        after_layout_change,
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

#[cfg(test)]
mod tests;
#[cfg(test)]
mod u7_probe;
