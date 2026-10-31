//! Routes decoded host events to current Client capture owners and terminal forwarding.
//! Workbench owns the input port and feature state; terminal events own pure byte parsing.

use crate::client_view::ClientView;
pub use crate::terminal_events::{
    DecodedItem, DecodedKind, HostReply, LegacyDecoder, MouseAction, MouseButton, MouseEvent,
    PaneEncode, PaneMouseEvent, encode_cursor_arrows, encode_pane_report,
};
use crate::terminal_events::{decoder, mouse, utf8};
use crate::workbench_client::RenameEdit;
pub use crate::workbench_client::input::{
    BoardAction, CopyAction, LayoutAction, MenuAction, RenameAction, SettingsAction, SetupAction,
    SurfaceAction, TabAction, WorktreeAction,
};
#[cfg(any(test, feature = "test-support"))]
pub use crate::workbench_client::input::{
    CloseDecision, PointerFrameFacts, PointerPane, PointerProbe, PointerSelectionIntent,
    PointerTab, TerminalInputUnavailable,
};
pub use crate::workbench_client::input::{
    ModeRequest, PaneAdmission, PaneInput, PointerCaptureRequest, SurfaceInput, TerminalInputPort,
    TerminalObservation, TextEntry, UnavailableTerminalInputPort,
};
use crate::workbench_state::rename_draft::RenameDraftToken;
use crate::workbench_state::workspace_setup::{SetupEdit, SetupToken};
mod dispatcher;
mod modal;
pub use dispatcher::InputDispatcher;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchOutcome {
    Continue,
    Exit,
}

/// Test helper: drives the real `InputDispatcher` path so tests exercise the
/// same decode → ownership → dispatch rules as production, instead of a
/// parallel byte-level loop.
#[cfg(any(test, feature = "test-support"))]
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

fn forward_edit_or_report(
    view: &mut ClientView,
    input: &[u8],
    editing: bool,
    terminal_input: &mut (impl TerminalInputPort + ?Sized),
) {
    match terminal_input.forward(input) {
        Ok(()) if editing => terminal_input.observe_terminal(TerminalObservation::TerminalEdited),
        Ok(()) => {}
        Err(_) => view.report_terminal_input_unavailable(),
    }
}

#[cfg(test)]
mod tests;
