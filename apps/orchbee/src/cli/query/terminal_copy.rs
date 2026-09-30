//! Public copy commands project one complete admitted physical buffer read.
use super::terminal_buffer::{self, BufferReader, ContentFence, Prepared};
use super::viewer_protocol_invalid;
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
                        text: locale.cli_terminal_copy_motion(
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
                    let text = locale.cli_terminal_copy_search(
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
    let row = u64::from(cursor.row);
    if matches!(kind, CopyMotion::LineEnd | CopyMotion::FirstNonBlank) {
        let rows = reader.rows(row..row + 1)?;
        return Ok(terminal_copy::motion(
            &rows,
            reader.frame.geometry.columns,
            cursor,
            kind,
        ));
    }
    let paragraph = matches!(
        kind,
        CopyMotion::PreviousParagraph | CopyMotion::NextParagraph
    );
    let (minimum, maximum) = if paragraph {
        (
            row.saturating_sub(999),
            row.saturating_add(1000).min(reader.total),
        )
    } else {
        (0, reader.total)
    };
    let backward = kind.backward();
    let mut start = if backward {
        row.saturating_sub(63).max(minimum)
    } else {
        row
    };
    let mut end = if backward {
        row + 1
    } else {
        row.saturating_add(64).min(maximum)
    };
    let mut rows = reader.rows(start..end)?;
    loop {
        let target = terminal_copy::motion(&rows, reader.frame.geometry.columns, cursor, kind);
        let edge = if backward {
            start == minimum
        } else {
            end == maximum
        };
        let cut_start = !paragraph
            && backward
            && rows
                .first()
                .is_some_and(|first| first.wrap_continuation && target.row as u64 == first.index);
        let cut_end = matches!(kind, CopyMotion::NextWordEnd | CopyMotion::NextBigWordEnd)
            && rows
                .last()
                .is_some_and(|last| last.wrap && target.row as u64 == last.index);
        if edge || (target != cursor && !cut_start && !cut_end) {
            return Ok(target);
        }
        let remaining = 65536_usize.saturating_sub(rows.len()) as u64;
        let extension = (rows.len() as u64).min(remaining);
        if extension == 0 {
            return Err(terminal_buffer::limit());
        }
        if backward {
            let next = start.saturating_sub(extension).max(minimum);
            let mut incoming = reader.rows(next..start)?;
            if let (Some(prior), Some(after)) = (incoming.last(), rows.first())
                && prior.wrap != after.wrap_continuation
            {
                return Err(viewer_protocol_invalid());
            }
            incoming.extend(rows);
            rows = incoming;
            start = next;
        } else {
            let next = end.saturating_add(extension).min(maximum);
            let incoming = reader.rows(end..next)?;
            if let (Some(prior), Some(after)) = (rows.last(), incoming.first())
                && prior.wrap != after.wrap_continuation
            {
                return Err(viewer_protocol_invalid());
            }
            rows.extend(incoming);
            end = next;
        }
    }
}

fn point(point: TerminalTextPoint) -> Value {
    json!({"row":point.row,"col":point.col})
}
fn range(range: TerminalTextRange) -> Value {
    json!({"start":point(range.start),"end":point(range.end)})
}

#[cfg(test)]
mod tests;
