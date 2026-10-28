//! Routes decoded host events to current Client capture owners and terminal forwarding.
//! Workbench owns the input port and feature state; terminal events own pure byte parsing.

use crate::client_view::ClientView;
pub use crate::terminal_events::HostReply;
pub use crate::terminal_events::{
    DecodedItem, DecodedKind, LegacyDecoder, MouseAction, MouseButton, MouseEvent, PaneEncode,
    PaneMouseEvent, encode_cursor_arrows, encode_pane_report,
};
pub(crate) use crate::terminal_events::{PanePointerRoute, route_pane_pointer};
use crate::terminal_events::{decoder, mouse, utf8};
pub use crate::workbench_client::input::{
    ModeRequest, PointerCaptureRequest, TerminalInputPort, UnavailableTerminalInputPort,
};
pub use crate::workbench_client::input::{
    PointerFrameFacts, PointerProbe, PointerSelectionAdmission, PointerSelectionIntent,
};
pub(crate) use crate::workbench_client::input::{PointerSelectionCell, PointerUnboundPane};
// Existing dispatcher fixtures build pointer facts; production consumers use the owning port.
#[cfg(test)]
pub(crate) use crate::workbench_client::input::PointerLinkAdmission;
#[cfg(any(test, feature = "test-support"))]
pub use crate::workbench_client::input::{PointerPane, PointerTab, TerminalInputUnavailable};
use crate::workbench_client::{RenameDraftToken, RenameEdit, SetupEdit, SetupToken};
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
