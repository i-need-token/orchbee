use crate::client_view::{ClientAction, ClientKey, ClientView, SessionActionDisposition};
use crate::terminal_session::{NativeRenameDraftToken, NativeRenameEdit, NativeRenameInputContext};
use crate::terminal_session::{
    NativeSetupEdit, NativeSetupInputContext, NativeSetupStage, NativeSetupToken,
};
use orchbee_client_server_protocol::Geometry;

mod decoder;
mod dispatcher;
mod modal;
pub(crate) use dispatcher::InputDispatcher;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalInputUnavailable;

pub(crate) trait TerminalInputPort {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable>;

    fn apply_client_action(&mut self, _action: ClientAction) -> Option<SessionActionDisposition> {
        None
    }

    fn observe_geometry(&mut self, _geometry: Geometry) {}

    fn capture_confirmation_key(&mut self, _key: ClientKey) -> bool {
        false
    }

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

fn forward_or_report(
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
