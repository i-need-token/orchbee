use crate::client_view::{
    ClientAction, ClientKey, ClientView, SessionActionDisposition, ShellCommand,
};
use crate::terminal_session::{NativeRenameDraftToken, NativeRenameEdit, NativeRenameInputContext};
use orchbee_client_server_protocol::Geometry;

const CONTROL_B: u8 = 0x02;

mod dispatcher;
mod rename;
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

#[cfg(test)]
pub(crate) fn dispatch_input<Port: TerminalInputPort + ?Sized, LayoutError>(
    view: &mut ClientView,
    input: &[u8],
    terminal_input: &mut Port,
    mut after_layout_change: impl FnMut(&mut ClientView, &mut Port) -> Result<(), LayoutError>,
) -> Result<DispatchOutcome, LayoutError> {
    let mut ordinary_start = 0;
    let mut offset = 0;
    let mut modal_packet = false;

    while let Some(byte) = input.get(offset).copied() {
        if terminal_input.capture_confirmation_key(normalize_byte(byte)) || modal_packet {
            if let Some(ordinary) = input.get(ordinary_start..offset)
                && !ordinary.is_empty()
            {
                forward_or_report(view, ordinary, terminal_input);
            }
            modal_packet = true;
            offset += 1;
            ordinary_start = offset;
            continue;
        }
        if !view.prefix_active() && byte != CONTROL_B {
            offset += 1;
            continue;
        }

        if let Some(ordinary) = input.get(ordinary_start..offset)
            && !ordinary.is_empty()
        {
            forward_or_report(view, ordinary, terminal_input);
        }
        let transition = view.decide(normalize_byte(byte));
        let action = transition.action();
        let applied = action.and_then(|action| terminal_input.apply_client_action(action));
        let disposition = applied.unwrap_or(SessionActionDisposition::KeepNavigator);
        view.commit(transition, disposition);
        if applied.is_none()
            && let Some(action) = action.and_then(ClientAction::backend_action)
        {
            view.report_backend_unavailable(action);
        }
        if transition.layout_changed() {
            after_layout_change(view, terminal_input)?;
        }
        match transition.shell() {
            ShellCommand::Exit => return Ok(DispatchOutcome::Exit),
            ShellCommand::ForwardTerminal => {
                forward_or_report(view, std::slice::from_ref(&byte), terminal_input);
            }
            ShellCommand::None => {}
        }
        offset += 1;
        ordinary_start = offset;
    }

    if let Some(ordinary) = input.get(ordinary_start..)
        && !ordinary.is_empty()
    {
        forward_or_report(view, ordinary, terminal_input);
    }
    Ok(DispatchOutcome::Continue)
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

const fn normalize_byte(byte: u8) -> ClientKey {
    match byte {
        CONTROL_B => ClientKey::ControlB,
        0x1b => ClientKey::Escape,
        byte if byte.is_ascii() => ClientKey::Plain(byte as char),
        _ => ClientKey::Other,
    }
}

#[cfg(test)]
mod tests;
