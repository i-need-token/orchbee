//! One revision-pinned buffer read; it does not own the user's selection.
use super::terminal_buffer::{self, ContentFence, Prepared};
use super::viewer_protocol_invalid;
use crate::{
    cli::{Failure, Output},
    invocation::{OutputFormat, TerminalSelectionCommand, TerminalTextPoint},
    localization::Locale,
};
use orchbee_terminal_protocol::TerminalTextRowWindow;
use serde_json::{Value, json};

pub(crate) fn run(
    command: &TerminalSelectionCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    terminal_buffer::run(
        &command.target,
        ContentFence {
            revision: command.revision,
            generation: command.generation,
        },
        format,
        locale,
        "orchbee.terminal.selection",
        |reader| {
            reader.validate_point(command.anchor)?;
            reader.validate_point(command.cursor)?;
            let (start, end) = ordered(command.anchor, command.cursor);
            let count = (u64::from(end.row) - u64::from(start.row) + 1) as u16;
            let window = reader.window(u64::from(start.row), count)?;
            let text = extract(&window, start, end)?;
            let rendered = locale.cli_terminal_selection(
                &reader.terminal.terminal_id,
                reader.frame.route.revision,
                &text,
            );
            Ok(Prepared {
                status: 0,
                submitted: false,
                result: json!({"anchor":point(command.anchor),"cursor":point(command.cursor),"text":text}),
                text: rendered,
            })
        },
    )
}

fn ordered(a: TerminalTextPoint, b: TerminalTextPoint) -> (TerminalTextPoint, TerminalTextPoint) {
    if a <= b { (a, b) } else { (b, a) }
}
fn point(point: TerminalTextPoint) -> Value {
    json!({"row":point.row,"col":point.col})
}

fn extract(
    window: &TerminalTextRowWindow,
    a: TerminalTextPoint,
    b: TerminalTextPoint,
) -> Result<String, Failure> {
    orchbee_terminal_protocol::extract_terminal_text_selection(
        window,
        orchbee_terminal_protocol::SelectionPoint {
            row: u64::from(a.row),
            column: a.col,
        },
        orchbee_terminal_protocol::SelectionPoint {
            row: u64::from(b.row),
            column: b.col,
        },
        8 * 1024 * 1024,
    )
    .map_err(|_| viewer_protocol_invalid())
}

#[cfg(test)]
mod tests;
