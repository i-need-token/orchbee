//! Public copy commands project one complete admitted physical buffer read.
use super::terminal_buffer::{self, BufferReader, ContentFence, Prepared};
use super::viewer_protocol_invalid;
use crate::cli_text::CliText;
use crate::{
    cli::{Failure, FailureKind, Output},
    invocation::{OutputFormat, TerminalCopyAction, TerminalCopyCommand},
    localization::Locale,
    terminal_copy::{self, CopyError, CopyMotion, TerminalTextPoint, TerminalTextRange},
};
use serde_json::{Value, json};

pub(crate) fn run(command: &TerminalCopyCommand, format: OutputFormat, locale: Locale) -> Output {
    let schema = match command.action {
        TerminalCopyAction::Motion { .. } => "orchbee.terminal.copy-motion",
        TerminalCopyAction::Search { .. } => "orchbee.terminal.copy-search",
    };
    terminal_buffer::run(
        &command.target,
        ContentFence {
            revision: command.revision(),
            generation: command.generation,
        },
        format,
        locale,
        schema,
        |reader| {
            let cursor = command.cursor();
            reader.validate_point(cursor)?;
            match &command.action {
                TerminalCopyAction::Motion { motion, .. } => {
                    let target = read_motion(reader, cursor, *motion)?;
                    Ok(Prepared {
                        status: 0,
                        submitted: false,
                        result: json!({"origin":point(cursor),"motion":motion.token(),"cursor":point(target)}),
                        text: CliText::new(locale).cli_terminal_copy_motion(
                            &reader.terminal.terminal_id,
                            reader.frame.route.revision,
                            motion.token(),
                            cursor,
                            target,
                        ),
                    })
                }
                TerminalCopyAction::Search {
                    query,
                    direction,
                    previous,
                    ..
                } => {
                    if let Some(previous) = previous {
                        reader.validate_point(previous.start)?;
                        reader.validate_point(previous.end)?;
                    }
                    let rows = if query.is_empty() {
                        Vec::new()
                    } else {
                        reader.rows(0..reader.total)?
                    };
                    let found = terminal_copy::search(
                        &rows,
                        reader.frame.geometry.columns,
                        query,
                        terminal_copy::CopySearchCase::Smart,
                        *direction,
                        cursor,
                        *previous,
                    )
                    .map_err(|error| match error {
                        CopyError::LimitExceeded => terminal_buffer::limit(),
                        CopyError::PatternTooComplex => {
                            crate::cli::Failure::new(FailureKind::InvalidUsage)
                        }
                    })?;
                    let text = CliText::new(locale).cli_terminal_copy_search(
                        &reader.terminal.terminal_id,
                        reader.frame.route.revision,
                        query,
                        &found,
                    );
                    Ok(Prepared {
                        status: 0,
                        submitted: false,
                        result: json!({
                            "origin":point(cursor),"query":query,"direction":direction.token(),
                            "previous":previous.map(range),
                            "matches":found.matches.into_iter().map(range).collect::<Vec<_>>(),
                            "total":found.total.to_string(),"current":found.current,
                            "currentGlobal":found.current_global.map(|index|index.to_string()),
                        }),
                        text,
                    })
                }
            }
        },
    )
}
// A motion needs enough context to prove a boundary, not every unrelated history row.
fn read_motion(
    reader: &mut BufferReader<'_>,
    cursor: TerminalTextPoint,
    kind: CopyMotion,
) -> Result<TerminalTextPoint, Failure> {
    let columns = reader.frame.geometry.columns;
    let total = reader.total;
    terminal_copy::read_motion_with(
        columns,
        total,
        cursor,
        kind,
        |range| reader.rows(range),
        terminal_buffer::limit,
        viewer_protocol_invalid,
    )
}

fn point(point: TerminalTextPoint) -> Value {
    json!({"row":point.row,"col":point.col})
}
fn range(range: TerminalTextRange) -> Value {
    json!({"start":point(range.start),"end":point(range.end)})
}

#[cfg(test)]
mod tests;
