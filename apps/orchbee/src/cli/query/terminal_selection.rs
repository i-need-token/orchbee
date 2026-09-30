//! One revision-pinned buffer read; it does not own the user's selection.
use super::terminal_buffer::{self, ContentFence, Prepared};
use super::viewer_protocol_invalid;
use crate::{
    cli::{Failure, Output},
    invocation::{OutputFormat, TerminalSelectionCommand, TerminalTextPoint},
    localization::Locale,
};
use orchbee_terminal_protocol::{TerminalTextRowWindow, TerminalTextWindowRow};
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
    let (start, end) = ordered(a, b);
    if window.columns == 0
        || start.col >= window.columns
        || end.col >= window.columns
        || window.start_row != u64::from(start.row)
        || u64::from(end.row) >= window.total_rows
        || window.rows.len() as u64 != u64::from(end.row) - u64::from(start.row) + 1
    {
        return Err(viewer_protocol_invalid());
    }
    let mut text = String::new();
    for (index, row) in window.rows.iter().enumerate() {
        if row.index != u64::from(start.row) + index as u64 {
            return Err(viewer_protocol_invalid());
        }
        let first = if row.index == u64::from(start.row) {
            start.col
        } else {
            0
        };
        let last = if row.index == u64::from(end.row) {
            end.col
        } else {
            window.columns - 1
        };
        let part = slice(row, first, last)?;
        let joined = row.wrap
            && window
                .rows
                .get(index + 1)
                .is_some_and(|next| next.wrap_continuation);
        text.push_str(if joined {
            part
        } else {
            part.trim_end_matches(' ')
        });
        if index + 1 < window.rows.len() && !joined {
            text.push('\n');
        }
    }
    Ok(text)
}
fn slice(row: &TerminalTextWindowRow, first: u16, last: u16) -> Result<&str, Failure> {
    if row.identity_ascii {
        return row
            .text
            .get(
                usize::from(first).min(row.text.len())..(usize::from(last) + 1).min(row.text.len()),
            )
            .ok_or_else(viewer_protocol_invalid);
    }
    let intersects = |index: usize| {
        let cell = row.mappings[index];
        cell.column <= last && u32::from(cell.column) + u32::from(cell.width) > u32::from(first)
    };
    let Some(start) = (0..row.mappings.len()).find(|index| intersects(*index)) else {
        return Ok("");
    };
    let end = (start..row.mappings.len())
        .take_while(|index| intersects(*index))
        .last()
        .unwrap_or(start);
    let from = row.mappings[start].byte_offset as usize;
    let to = row
        .mappings
        .get(end + 1)
        .map_or(row.text.len(), |cell| cell.byte_offset as usize);
    row.text.get(from..to).ok_or_else(viewer_protocol_invalid)
}

#[cfg(test)]
mod tests;
