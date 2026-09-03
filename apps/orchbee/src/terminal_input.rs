use crate::client_view::{ClientKey, ClientView, ShellCommand};

const CONTROL_B: u8 = 0x02;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalInputUnavailable;

pub(crate) trait TerminalInputPort {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable>;
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

pub(crate) fn dispatch_input(
    view: &mut ClientView,
    input: &[u8],
    terminal_input: &mut impl TerminalInputPort,
) -> DispatchOutcome {
    let mut ordinary_start = 0;
    let mut offset = 0;

    while let Some(byte) = input.get(offset).copied() {
        if !view.prefix_active() && byte != CONTROL_B {
            offset += 1;
            continue;
        }

        if let Some(ordinary) = input.get(ordinary_start..offset)
            && !ordinary.is_empty()
        {
            forward_or_report(view, ordinary, terminal_input);
        }
        match view.transition(normalize_byte(byte)) {
            ShellCommand::Backend(action) => view.report_backend_unavailable(action),
            ShellCommand::Exit => return DispatchOutcome::Exit,
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
    DispatchOutcome::Continue
}

fn forward_or_report(
    view: &mut ClientView,
    input: &[u8],
    terminal_input: &mut impl TerminalInputPort,
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
